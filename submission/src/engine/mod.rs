use std::collections::HashMap;
use super::model::order::{Symbol, OrderKey, OrderLocation, Orders};
use super::model::book::{OrderBook};
use super::model::command::{Command, NewOrder, CancelOrder};
use super::model::event::{Event, Events};

pub enum EngineError {
    DuplicateOrder(OrderKey),
    UnknownOrder(OrderKey),
    QuantityOverflow,
    OrderCapacityExceeded,
}

// main struct and entry point for each order book command. New, Cancel, Flush commands might mutate
// the book state. All engine business rule apply to this struct
pub struct OrderBookEngine {
    books: HashMap<Symbol, OrderBook>,
    order_locations: HashMap<OrderKey, OrderLocation>,
    orders: Orders,
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
        Ok(Events::None)
    }

    fn process_cancel(&mut self, cancel: CancelOrder) -> Result<Events, EngineError> {
        Ok(Events::None)
    }

    fn process_flush(&mut self) -> Result<Events, EngineError> {
        Ok(Events::None)
    }
}

impl Default for OrderBookEngine {
    fn default() -> Self {
        Self::new()
    }
}