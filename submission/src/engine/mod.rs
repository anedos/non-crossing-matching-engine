use std::collections::HashMap;
use super::model::order::{Symbol, OrderKey, Orders, Side, Price, OrderIndex, StoredOrder};
use super::model::book::{OrderBook, PriceLevel};
use super::model::command::{Command, NewOrder, CancelOrder};
use super::model::event::{Event, Events};

pub enum EngineError {
    DuplicateOrder(OrderKey),
    UnknownOrder(OrderKey),
    QuantityOverflow,
    OrderCapacityExceeded,
}

// OrderLocation only contains sufficient fields to identify the right OrderBook - based on
// symbol, side and price and the index to Orders > OrderSlot > StoredOrder
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct OrderLocation {
    pub symbol: Symbol,
    pub side: Side,
    pub price: Price,
    pub order_index: OrderIndex,
}

impl OrderLocation {
    pub fn new(symbol: Symbol, side: Side, price: Price, order_index: OrderIndex) -> Self {
        Self {
            symbol,
            side,
            price,
            order_index,
        }
    }
}

// main struct and entry point for each order book command. New, Cancel, Flush commands might mutate
// the book state. All engine business rule apply to this struct
pub struct OrderBookEngine {
    books: HashMap<Symbol, OrderBook>,
    order_locations: HashMap<OrderKey, OrderLocation>,
    orders: Orders,
}

impl Default for OrderBookEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBookEngine {
    pub fn process(
        &mut self,
        command: Command
    ) -> Result<Events, EngineError> {
        let events = match command {
            Command::New(order) => self.process_new(order),
            Command::Cancel(cancel) => self.process_cancel(cancel),
            Command::Flush => self.process_flush(),
        }?;

        // iterate over events and write them
        Ok(events)
    }

    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
            order_locations: HashMap::new(),
            orders: Orders::new(),
        }
    }

    fn process_new(&mut self, order: NewOrder) -> Result<Events, EngineError> {
        // 1. Validate the order identity.
        //    - Build an OrderKey from user_id and user_order_id.
        //    - Return DuplicateOrder if that key is already present in order_locations. The
        //      spec does not define duplicate-order rejection, so I'm treating it as an engine
        //      error rather than emitting Rejected.
        //
        // 2. Check whether the order crosses without mutating state.
        //    - Inspect the symbol's existing book; do not create a book yet.
        //    - A bid crosses when its price is at least the lowest ask.
        //    - An ask crosses when its price is at most the highest bid.
        //    - A missing book or empty opposing side cannot cross.
        //    - If the order crosses, emit only Rejected. Do not create a book, allocate an order
        //      slot, modify a price level, or add an order location.
        //
        // 3. Insert a non-crossing order.
        //    - Get or create the symbol's book and select the map for the order's side.
        //    - Capture the current top so it can be compared with the top after insertion.
        //    - Get or create the price level, then check that adding the quantity will not overflow.
        //    - Create a StoredOrder whose previous link is the price level's current last order.
        //    - Insert it into Orders: reuse first_free and advance the free list when possible;
        //      otherwise, append a new occupied slot. Retain the resulting OrderIndex.
        //    - Link the previous order to the new index, by setting previous_order.next = new_idx,
        //      if the level was empty, also set first.
        //    - Set the level's last index and total quantity.
        //    - Add an OrderLocation containing the symbol, side, price, and index, keyed by the
        //      order's OrderKey.
        //
        // 4. Emit Accepted, followed by TopChanged when the best price or aggregate quantity
        //    changed.

        let order_key = OrderKey::new(order.user_id, order.user_order_id);

        // exit if we receive a duplicated order key
        if self.order_locations.contains_key(&order_key) {
            return Err(EngineError::DuplicateOrder(order_key));
        }

        // emit a Rejected event for any order that crosses
        if let Some(book) = self.books.get(&order.symbol) {
            if book.crosses(order.side, order.price) {
                return Ok(Events::One(Event::Rejected {
                    user_id: order.user_id,
                    user_order_id: order.user_order_id
                }));
            }
        }

        // get or create the symbol's book and get the map for the order's side
        let book: &mut OrderBook = self.books.entry(order.symbol.clone()).or_insert(OrderBook::new());

        // get the current top before the book mutates
        let current_top = book.get_top(order.side, order.price);

        // --> start mutating operations <--
        let price_level = book
            .get_by_side_mut(order.side)
            .entry(order.price)
            .or_default();

        // check price level does not overflow
        let new_total_quantity = price_level.check_total_after_add(order.quantity)
            .ok_or(EngineError::QuantityOverflow)?;

        let idx = self.orders
            .append_after(price_level.last(), order.quantity)
            .map_err(|_| EngineError::QuantityOverflow)?;

        // Set the level's last index and total quantity.
        price_level.add_order(idx, order.quantity);

        // Capture the updated top before finishing with book.
        let new_top = book.get_top(order.side, order.price);

        self.add_order_location(order_key, order.symbol, order.side, order.price, idx);

        if new_top == current_top {
            Ok(Events::One(
                Event::Accepted {
                    user_id: order.user_id,
                    user_order_id: order.user_order_id
                }
            ))
        } else {
            Ok(Events::Two(
                Event::Accepted {
                    user_id: order.user_id,
                    user_order_id: order.user_order_id
                },
                Event::TopChanged {
                    side: order.side,
                    top: new_top
                }
            ))
        }
    }

    fn process_cancel(&mut self, cancel: CancelOrder) -> Result<Events, EngineError> {
        // 1. Locate the order.
        //    - Build an OrderKey from user_id and user_order_id and look it up in order_locations.
        //    - Return UnknownOrder when the key is absent. The protocol does not define an output
        //      event for an unknown cancellation, so treat it as an engine error.
        //    - Use the OrderLocation to identify the symbol's book, side, price level, and exact
        //      OrderSlot through order_index.
        //    - Verify that the slot is Occupied. A Free slot indicates an internal consistency
        //      error.
        //
        // 2. Capture the current top, quantity, previous index, and next index, then unlink the
        //    order from its price-level FIFO.
        //    - Link the previous order to the cancelled order's next index, or set
        //      price_level.first when the cancelled order was first.
        //    - Link the next order to the cancelled order's previous index, or set
        //      price_level.last when the cancelled order was last.
        //    - Subtract the cancelled quantity from total_quantity using checked arithmetic.
        //
        // 3. Clean up the remaining state.
        //    - If the price level is empty, verify that first and last are None, then remove the
        //      level from the appropriate bid or ask map.
        //    - Remove the order's entry from order_locations.
        //    - Replace the occupied slot with a Free slot and push it onto the free-list head:
        //
        //      slots[order_index] = OrderSlot::Free {
        //          next_free: orders.first_free,
        //      };
        //      orders.first_free = Some(order_index);
        //
        //      Do not traverse the free list.
        //
        // 4. Compare the affected side's new top with its previous top. Return Accepted for the
        //    cancellation, followed by TopChanged when the best price or aggregate quantity
        //    changed.
        Ok(Events::None)
    }

    fn process_flush(&mut self) -> Result<Events, EngineError> {
        Ok(Events::None)
    }

    fn add_order_location(
        &mut self,
        order_key: OrderKey,
        symbol: Symbol,
        side: Side,
        price: Price,
        order_index: OrderIndex) {
        self.order_locations.insert(order_key, OrderLocation::new(symbol, side, price, order_index));
    }
}
