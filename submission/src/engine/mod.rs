use ahash::AHashMap as HashMap;
use anyhow::{Result};

use super::model::order::{Symbol, OrderKey, Orders, Side, Price, OrderIndex};
use super::model::book::{OrderBook, TopOfBook};
use super::model::command::{Command, NewOrder, CancelOrder};
use super::model::event::{Event, Events};

#[derive(Debug)]
pub enum EngineError {
    DuplicateOrder(OrderKey),
    UnknownOrder(OrderKey),
    QuantityOverflow,
    OrderCapacityExceeded,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateOrder(key) => write!(f, "duplicate order: {key:?}"),
            Self::UnknownOrder(key) => write!(f, "unknown order: {key:?}"),
            Self::QuantityOverflow => f.write_str("price level quantity overflow"),
            Self::OrderCapacityExceeded => f.write_str("order capacity exceeded"),
        }
    }
}

impl std::error::Error for EngineError {}

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
        log::debug!("processing command: {command:?}");
        let events = match command {
            Command::New(order) => self.process_new(order),
            Command::Cancel(cancel) => self.process_cancel(cancel),
            Command::Flush => self.process_flush(),
        }?;

        log::debug!("command completed: events={events:?}");
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
        //    - Read the existing book and level without creating either one.
        //    - Capture the current top, previous tail, and checked new total quantity.
        //    - Create a StoredOrder whose previous link is the price level's current last order.
        //    - Insert it into Orders: reuse first_free and advance the free list when possible;
        //      otherwise, append a new occupied slot. Retain the resulting OrderIndex.
        //    - Link the previous order to the new index, by setting previous_order.next = new_idx,
        //      if the level was empty, also set first.
        //    - After allocation succeeds, get or create the book and level, then commit the
        //      endpoints and checked total quantity. Errors must not leave empty levels behind.
        //    - Add an OrderLocation containing the symbol, side, price, and index, keyed by the
        //      order's OrderKey.
        //
        // 4. Emit Accepted, followed by TopChanged when the best price or aggregate quantity
        //    changed.

        let order_key = OrderKey::new(order.user_id, order.user_order_id);

        // exit if we receive a duplicated order key
        if self.order_locations.contains_key(&order_key) {
            log::warn!("new order failed: key={order_key:?} symbol={} reason=DuplicateOrder", order.symbol);
            return Err(EngineError::DuplicateOrder(order_key));
        }

        // Reuse the lookup for crossing validation and the existing price level.
        let book = self.books.get(&order.symbol);
        if let Some(book) = book {
            if book.crosses(order.side, order.price) {
                return Ok(Events::One(Event::Rejected {
                    user_id: order.user_id,
                    user_order_id: order.user_order_id
                }));
            }
        }

        // Validate against existing state before allocating an order or creating map entries.
        let current_top = book.and_then(|book| book.get_top(order.side));
        let price_level = book.and_then(|book| book.get_by_side(order.side).get(&order.price));
        let (previous, new_total_quantity) = match price_level {
            Some(level) => (
                level.last(),
                level.check_total_after_add(order.quantity)
                    .ok_or_else(|| {
                        log::warn!("new order failed: key={order_key:?} symbol={} price={} reason=QuantityOverflow", order.symbol, order.price);
                        EngineError::QuantityOverflow
                    })?,
            ),
            None => (None, order.quantity),
        };

        let idx = self.orders
            .append_after(previous, order.quantity)
            .map_err(|_| {
                log::warn!("new order failed: key={order_key:?} symbol={} reason=OrderCapacityExceeded", order.symbol);
                EngineError::OrderCapacityExceeded
            })?;

        // All recoverable errors have been checked; commit the book state.
        let book = self.books.entry(order.symbol.clone()).or_default();
        book.get_by_side_mut(order.side)
            .entry(order.price)
            .or_default()
            .commit_add(idx, new_total_quantity);

        // Capture the updated top before finishing with book.
        let new_top = book.get_top(order.side);

        self.add_order_location(order_key, order.symbol, order.side, order.price, idx);

        let accepted = Event::Accepted {
            user_id: order.user_id,
            user_order_id: order.user_order_id,
        };
        Ok(Self::accepted_events(accepted, order.side, current_top, new_top))
    }

    fn process_cancel(&mut self, cancel: CancelOrder) -> Result<Events, EngineError> {
        // 1. Locate the order without mutating state.
        //    - Build an OrderKey from user_id and user_order_id.
        //    - Return UnknownOrder if the key is absent from order_locations.
        //    - Use the location to find the stored order, symbol's book, side, and price level.
        //    - Capture the current top and the order's quantity, previous, and next links.
        //    - Check quantity subtraction before removal. Missing internal entries or quantity
        //      underflow indicate inconsistent engine state.
        //
        // 2. Unlink the order and update its price level.
        //    - Connect the previous order's next link to the cancelled order's next.
        //    - Connect the next order's previous link to the cancelled order's previous.
        //    - Replace the cancelled slot with Free and push it onto the free-list head.
        //    - Update the level's first or last index when removing an endpoint.
        //    - Assign the checked remaining total quantity.
        //
        // 3. Clean up empty entries.
        //    - Remove the price level when no orders remain. Determine emptiness from its
        //      endpoints, since zero-quantity orders can still occupy the level.
        //    - Capture the updated top, then remove the symbol's book if both sides are empty.
        //    - Remove the cancelled order's identity from order_locations.
        //
        // 4. Emit Accepted, followed by TopChanged when the best price or aggregate quantity
        //    changed. Use top: None when the affected side becomes empty.

        let order_key = OrderKey::new(cancel.user_id, cancel.user_order_id);
        let location = self.order_locations.get(&order_key)
            .ok_or_else(|| {
                log::warn!("cancellation failed: key={order_key:?} reason=UnknownOrder");
                EngineError::UnknownOrder(order_key)
            })?
            .clone();
        log::debug!("cancelling order: key={order_key:?} location={location:?}");
        let order = *self.orders.get(location.order_index);
        let book = self.books.get_mut(&location.symbol)
            .expect("active order references a missing book");
        let current_top = book.get_top(location.side);
        let book_side = book.get_by_side_mut(location.side);
        let level = book_side.get_mut(&location.price)
            .expect("active order references a missing price level");
        let new_total = level.check_total_after_remove(order.quantity)
            .expect("cancelled order quantity exceeds its price level total");

        self.orders.remove(location.order_index);
        level.commit_remove(order.previous, order.next, new_total);
        if level.is_empty() {
            book_side.remove(&location.price);
        }
        let new_top = book.get_top(location.side);
        let book_is_empty = book.get_by_side(Side::Bid).is_empty()
            && book.get_by_side(Side::Ask).is_empty();
        if book_is_empty {
            self.books.remove(&location.symbol);
        }
        self.order_locations.remove(&order_key);

        let accepted = Event::Accepted {
            user_id: cancel.user_id,
            user_order_id: cancel.user_order_id,
        };
        Ok(Self::accepted_events(accepted, location.side, current_top, new_top))
    }

    fn accepted_events(
        accepted: Event,
        side: Side,
        current_top: Option<TopOfBook>,
        new_top: Option<TopOfBook>,
    ) -> Events {
        if new_top == current_top {
            Events::One(accepted)
        } else {
            Events::Two(accepted, Event::TopChanged { side, top: new_top })
        }
    }

    fn process_flush(&mut self) -> Result<Events, EngineError> {
        log::debug!("flushing engine: books={} orders={}", self.books.len(), self.order_locations.len());
        self.books.clear();
        self.order_locations.clear();
        self.orders.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::order::{Quantity, StoredOrder, UserId, UserOrderId};

    mod cancel_flush;
    mod reference;

    type Snapshot = (
        HashMap<Symbol, OrderBook>,
        HashMap<OrderKey, OrderLocation>,
        Orders,
    );

    fn snapshot(engine: &OrderBookEngine) -> Snapshot {
        (
            engine.books.clone(),
            engine.order_locations.clone(),
            engine.orders.clone(),
        )
    }

    fn order(id: u64, symbol: &str, side: Side, price: Price, quantity: Quantity) -> NewOrder {
        NewOrder {
            user_id: UserId(7),
            user_order_id: UserOrderId(id),
            symbol: Symbol::from(symbol),
            side,
            price,
            quantity,
        }
    }

    fn assert_accepted(event: Event, id: u64) {
        match event {
            Event::Accepted { user_id, user_order_id } => {
                assert_eq!(user_id, UserId(7));
                assert_eq!(user_order_id, UserOrderId(id));
            }
            _ => panic!("expected Accepted"),
        }
    }

    fn assert_events(events: Events, id: u64, side: Side, top: Option<(Price, Quantity)>) {
        match (events, top) {
            (Events::One(accepted), None) => assert_accepted(accepted, id),
            (
                Events::Two(accepted, Event::TopChanged { side: actual_side, top: Some(actual_top) }),
                Some(expected_top),
            ) => {
                assert_accepted(accepted, id);
                assert_eq!(actual_side, side);
                assert_eq!(actual_top.inspect(), expected_top);
            }
            _ => panic!("unexpected event count, order, or top-change payload"),
        }
    }

    fn assert_location(
        engine: &OrderBookEngine,
        id: u64,
        symbol: &str,
        side: Side,
        price: Price,
    ) -> OrderIndex {
        let location = &engine.order_locations[&OrderKey::new(UserId(7), UserOrderId(id))];
        assert_eq!(location.symbol.as_ref(), symbol);
        assert_eq!(location.side, side);
        assert_eq!(location.price, price);
        location.order_index
    }

    #[test]
    fn first_order_creates_level_and_emits_accepted_then_top() {
        for side in [Side::Bid, Side::Ask] {
            let mut engine = OrderBookEngine::new();
            let events = engine.process_new(order(1, "BTC", side, 100, 5)).unwrap();
            assert_events(events, 1, side, Some((100, 5)));

            let idx = assert_location(&engine, 1, "BTC", side, 100);
            assert_eq!(engine.books.len(), 1);
            assert_eq!(engine.order_locations.len(), 1);
            let book = &engine.books["BTC"];
            assert_eq!(book.get_by_side(side).len(), 1);
            assert_eq!(book.get_by_side(side)[&100].inspect(), (5, Some(idx), Some(idx)));
            assert_eq!(engine.orders.inspect_order(idx), StoredOrder::new(5, None, None));
            let opposite = match side { Side::Bid => Side::Ask, Side::Ask => Side::Bid };
            assert!(book.get_by_side(opposite).is_empty());
        }
    }

    #[test]
    fn better_price_changes_top_to_new_levels_quantity() {
        for (side, better) in [(Side::Bid, 110), (Side::Ask, 90)] {
            let mut engine = OrderBookEngine::new();
            let initial = engine.process_new(order(1, "BTC", side, 100, 5)).unwrap();
            assert_events(initial, 1, side, Some((100, 5)));
            let events = engine.process_new(order(2, "BTC", side, better, 3)).unwrap();
            assert_events(events, 2, side, Some((better, 3)));
            assert_location(&engine, 2, "BTC", side, better);
            assert_eq!(engine.books["BTC"].get_by_side(side)[&100].inspect().0, 5);
        }
    }

    #[test]
    fn worse_price_is_stored_without_top_change() {
        for (side, worse) in [(Side::Bid, 90), (Side::Ask, 110)] {
            let mut engine = OrderBookEngine::new();
            let initial = engine.process_new(order(1, "BTC", side, 100, 5)).unwrap();
            assert_events(initial, 1, side, Some((100, 5)));
            let events = engine.process_new(order(2, "BTC", side, worse, 3)).unwrap();
            assert_events(events, 2, side, None);
            let idx = assert_location(&engine, 2, "BTC", side, worse);
            let book = &engine.books["BTC"];
            assert_eq!(book.get_by_side(side)[&worse].inspect(), (3, Some(idx), Some(idx)));
            assert_eq!(book.get_top(side).unwrap().inspect(), (100, 5));
        }
    }

    #[test]
    fn same_price_orders_update_total_and_keep_fifo_links() {
        for side in [Side::Bid, Side::Ask] {
            let mut engine = OrderBookEngine::new();
            for (id, quantity, total) in [(1, 5, 5), (2, 3, 8), (3, 7, 15)] {
                let events = engine.process_new(order(id, "BTC", side, 100, quantity)).unwrap();
                assert_events(events, id, side, Some((100, total)));
            }

            let indices = [1, 2, 3].map(|id| assert_location(&engine, id, "BTC", side, 100));
            assert_eq!(engine.order_locations.len(), 3);
            assert_eq!(
                engine.books["BTC"].get_by_side(side)[&100].inspect(),
                (15, Some(indices[0]), Some(indices[2])),
            );
            assert_eq!(engine.orders.inspect_order(indices[0]), StoredOrder::new(5, None, Some(indices[1])));
            assert_eq!(engine.orders.inspect_order(indices[1]), StoredOrder::new(3, Some(indices[0]), Some(indices[2])));
            assert_eq!(engine.orders.inspect_order(indices[2]), StoredOrder::new(7, Some(indices[1]), None));
        }
    }

    #[test]
    fn crossing_orders_are_rejected_without_mutation() {
        for (resting_side, incoming_side, crossing_prices) in [
            (Side::Ask, Side::Bid, [100, 101]),
            (Side::Bid, Side::Ask, [100, 99]),
        ] {
            let mut engine = OrderBookEngine::new();
            let initial = engine.process_new(order(1, "BTC", resting_side, 100, 5)).unwrap();
            assert_events(initial, 1, resting_side, Some((100, 5)));
            let before = snapshot(&engine);

            for price in crossing_prices {
                let events = engine.process_new(order(2, "BTC", incoming_side, price, 3)).unwrap();
                match events {
                    Events::One(Event::Rejected { user_id, user_order_id }) => {
                        assert_eq!(user_id, UserId(7));
                        assert_eq!(user_order_id, UserOrderId(2));
                    }
                    _ => panic!("expected only Rejected"),
                }
                assert_eq!(snapshot(&engine), before);
            }
        }
    }

    #[test]
    fn duplicate_identity_is_rejected_without_creating_another_book() {
        let mut engine = OrderBookEngine::new();
        let initial = engine.process_new(order(1, "BTC", Side::Bid, 100, 5)).unwrap();
        assert_events(initial, 1, Side::Bid, Some((100, 5)));
        let before = snapshot(&engine);

        let result = engine.process_new(order(1, "ETH", Side::Ask, 200, 3));
        assert!(matches!(result, Err(EngineError::DuplicateOrder(key))
            if key == OrderKey::new(UserId(7), UserOrderId(1))));
        assert_eq!(snapshot(&engine), before);
    }

    #[test]
    fn symbols_sides_and_price_levels_have_independent_orders() {
        let mut engine = OrderBookEngine::new();
        let inputs = [
            (1, "BTC", Side::Bid, 100, 2),
            (2, "BTC", Side::Ask, 110, 3),
            (3, "ETH", Side::Bid, 200, 4),
            (4, "ETH", Side::Ask, 210, 5),
            (5, "BTC", Side::Bid, 90, 6),
        ];
        for (id, symbol, side, price, quantity) in inputs {
            let events = engine.process_new(order(id, symbol, side, price, quantity)).unwrap();
            let expected_top = if id == 5 { None } else { Some((price, quantity)) };
            assert_events(events, id, side, expected_top);
            let idx = assert_location(&engine, id, symbol, side, price);
            assert_eq!(engine.orders.inspect_order(idx), StoredOrder::new(quantity, None, None));
        }

        let other_book = engine.books["ETH"].clone();
        let other_side = engine.books["BTC"].get_by_side(Side::Ask).clone();
        let other_level = engine.books["BTC"].get_by_side(Side::Bid)[&90].clone();
        let events = engine.process_new(order(6, "BTC", Side::Bid, 100, 7)).unwrap();
        assert_events(events, 6, Side::Bid, Some((100, 9)));
        assert_eq!(engine.books.len(), 2);
        assert_eq!(engine.order_locations.len(), 6);
        assert_eq!(engine.books["ETH"], other_book);
        assert_eq!(engine.books["BTC"].get_by_side(Side::Ask), &other_side);
        assert_eq!(engine.books["BTC"].get_by_side(Side::Bid)[&90], other_level);

        let first = assert_location(&engine, 1, "BTC", Side::Bid, 100);
        let last = assert_location(&engine, 6, "BTC", Side::Bid, 100);
        assert_eq!(engine.orders.inspect_order(first), StoredOrder::new(2, None, Some(last)));
        assert_eq!(engine.orders.inspect_order(last), StoredOrder::new(7, Some(first), None));
        for (id, symbol, side, price, quantity) in inputs.into_iter().skip(1) {
            let idx = assert_location(&engine, id, symbol, side, price);
            assert_eq!(engine.orders.inspect_order(idx), StoredOrder::new(quantity, None, None));
        }
    }

    #[test]
    fn maximum_total_succeeds_and_overflow_leaves_state_unchanged() {
        for side in [Side::Bid, Side::Ask] {
            let mut engine = OrderBookEngine::new();
            let initial = engine.process_new(order(1, "BTC", side, 100, Quantity::MAX - 5)).unwrap();
            assert_events(initial, 1, side, Some((100, Quantity::MAX - 5)));
            let events = engine.process_new(order(2, "BTC", side, 100, 5)).unwrap();
            assert_events(events, 2, side, Some((100, Quantity::MAX)));
            let before = snapshot(&engine);

            assert!(matches!(
                engine.process_new(order(3, "BTC", side, 100, 1)),
                Err(EngineError::QuantityOverflow),
            ));
            assert_eq!(snapshot(&engine), before);

            // The failed order's identity remains available at a different price.
            let worse = match side { Side::Bid => 90, Side::Ask => 110 };
            let events = engine.process_new(order(3, "BTC", side, worse, 1)).unwrap();
            assert_events(events, 3, side, None);
        }
    }

    #[test]
    fn allocation_failure_preserves_existing_levels_and_creates_no_entries() {
        for side in [Side::Bid, Side::Ask] {
            let better = match side { Side::Bid => 110, Side::Ask => 90 };
            for (seed_book, symbol, price, expected_total) in [
                (false, "BTC", 100, 2), // Entirely empty engine.
                (true, "BTC", 100, 7),  // Existing level and tail.
                (true, "BTC", better, 2), // New price in an existing book.
                (true, "ETH", 100, 2), // New symbol alongside an existing book.
            ] {
                let mut engine = OrderBookEngine::new();
                if seed_book {
                    let initial = engine.process_new(order(1, "BTC", side, 100, 5)).unwrap();
                    assert_events(initial, 1, side, Some((100, 5)));
                }
                let before = snapshot(&engine);
                engine.orders.fail_next_allocation();

                assert!(matches!(
                    engine.process_new(order(2, symbol, side, price, 2)),
                    Err(EngineError::OrderCapacityExceeded),
                ));
                assert_eq!(snapshot(&engine), before);

                // The same request can succeed after the allocation failure is gone.
                let events = engine.process_new(order(2, symbol, side, price, 2)).unwrap();
                assert_events(events, 2, side, Some((price, expected_total)));
                assert_location(&engine, 2, symbol, side, price);
            }
        }
    }
}
