use serde::{Deserialize};

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Deserialize)]
pub struct UserId(pub u64);

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Deserialize)]
pub struct UserOrderId(pub u64);

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct OrderIndex(pub u32);

pub type Symbol = std::rc::Rc<str>;
pub type Price = u64;
pub type Quantity = u64;
pub type TotalQuantity = u64;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug, Deserialize)]
pub enum Side {
    #[serde(rename = "B")]
    Bid,
    #[serde(rename = "S")]
    Ask,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct OrderKey {
    pub user_id: UserId,
    pub user_order_id: UserOrderId,
}


#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct StoredOrder {
    pub quantity: Quantity,
    pub previous: Option<OrderIndex>,
    pub next: Option<OrderIndex>,
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

#[derive(Clone, PartialEq, Eq, Debug)]
enum OrderSlot {
    Occupied(StoredOrder),
    Free {
        next_free: Option<OrderIndex>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Orders {
    slots: Vec<OrderSlot>,
    first_free: Option<OrderIndex>,
}

impl Orders {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            first_free: None,
        }
    }
}