use serde::Deserialize;

use super::order::{UserId, Symbol, Price, Quantity, Side, UserOrderId};

#[derive(Debug, Deserialize)]
pub struct NewOrder {
    pub user_id: UserId,
    pub symbol: Symbol,
    pub price: Price,
    pub quantity: Quantity,
    pub side: Side,
    pub user_order_id: UserOrderId,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrder {
    pub user_id: UserId,
    pub user_order_id: UserOrderId,
}

pub enum Command {
    New (NewOrder),
    Cancel (CancelOrder),
    Flush,
}
