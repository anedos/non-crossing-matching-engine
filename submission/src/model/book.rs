use std::collections::BTreeMap;
use super::order::{OrderIndex, TotalQuantity, Price};

#[derive(Debug)]
pub struct PriceLevel {
    total_quantity: TotalQuantity,
    first: Option<OrderIndex>,
    last: Option<OrderIndex>,
}

#[derive(Debug)]
pub struct OrderBook {
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
}

pub struct TopOfBook {
    price: Price,
    total_quantity: TotalQuantity,
}
