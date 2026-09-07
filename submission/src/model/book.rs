use std::collections::BTreeMap;
use super::order::{OrderIndex, TotalQuantity, Price, Side};

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct PriceLevel {
    total_quantity: TotalQuantity,
    first: Option<OrderIndex>,
    last: Option<OrderIndex>,
}

impl PriceLevel {
    pub fn new() -> Self {
        PriceLevel::default()
    }

    /// Commit an allocated order using the total validated by `check_total_after_add`.
    /// For a new, empty level, the checked total is the new order's quantity.
    pub fn commit_add(&mut self, order_idx: OrderIndex, checked_total: TotalQuantity) {
        self.total_quantity = checked_total;

        if self.first.is_none() {
            self.first = Some(order_idx);
        }

        self.last = Some(order_idx);
    }

    pub fn check_total_after_add(&self, quantity: TotalQuantity) -> Option<TotalQuantity> {
        self.total_quantity.checked_add(quantity)
    }

    pub fn check_total_after_remove(&self, quantity: TotalQuantity) -> Option<TotalQuantity> {
        self.total_quantity.checked_sub(quantity)
    }

    /// Commit removal using the cancelled order's original links and validated total.
    pub fn commit_remove(
        &mut self,
        previous: Option<OrderIndex>,
        next: Option<OrderIndex>,
        checked_total: TotalQuantity,
    ) {
        if previous.is_none() {
            self.first = next;
        }
        if next.is_none() {
            self.last = previous;
        }
        self.total_quantity = checked_total;
        if self.is_empty() {
            assert_eq!(self.total_quantity, 0, "empty price level has nonzero quantity");
        }
    }

    pub fn is_empty(&self) -> bool {
        assert_eq!(self.first.is_none(), self.last.is_none(), "price level endpoints disagree about emptiness");
        self.first.is_none()
    }

    pub fn last(&self) -> Option<OrderIndex> {
        self.last
    }

    #[cfg(test)]
    pub(crate) fn inspect(&self) -> (TotalQuantity, Option<OrderIndex>, Option<OrderIndex>) {
        (self.total_quantity, self.first, self.last)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct OrderBook {
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn crosses(&self, side: Side, price: Price) -> bool {
        // a bid order crosses when it is higher or equal to the lowest ask price
        // an ask order crosses when it is lower or equal to the highest bid price
        match side {
            Side::Bid => self.asks
                .first_key_value()
                .is_some_and(|(&best_ask, _)| price >= best_ask),
            Side::Ask => self.bids
                .last_key_value()
                .is_some_and(|(&best_bid, _)| price <= best_bid),
        }
    }

    pub fn get_by_side(&self, side: Side) -> &BTreeMap<Price, PriceLevel> {
        match side {
            Side::Bid => &self.bids,
            Side::Ask => &self.asks,
        }
    }

    pub fn get_by_side_mut(&mut self, side: Side) -> &mut BTreeMap<Price, PriceLevel> {
        match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        }
    }

    pub fn get_top(&self, side: Side, price: Price, ) -> Option<TopOfBook> {
        let book_side = self.get_by_side(side);
        let (&price, level) = match side {
            Side::Bid => book_side.last_key_value(),
            Side::Ask => book_side.first_key_value(),
        }?;

        Some(TopOfBook{
            price,
            total_quantity: level.total_quantity,
        })
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TopOfBook {
    price: Price,
    total_quantity: TotalQuantity,
}

#[cfg(test)]
impl TopOfBook {
    pub(crate) fn inspect(&self) -> (Price, TotalQuantity) {
        (self.price, self.total_quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book_with_prices(bids: &[Price], asks: &[Price]) -> OrderBook {
        let mut book = OrderBook::new();
        let mut order_index = 0;
        for (levels, prices) in [(&mut book.bids, bids), (&mut book.asks, asks)] {
            for &price in prices {
                let mut level = PriceLevel::new();
                level.commit_add(OrderIndex(order_index), 1);
                levels.insert(price, level);
                order_index += 1;
            }
        }
        book
    }

    #[test]
    fn empty_book_never_crosses() {
        let book = OrderBook::new();
        for side in [Side::Bid, Side::Ask] {
            for price in [0, 100, Price::MAX] {
                assert!(!book.crosses(side, price), "side={side:?}, price={price}");
            }
        }
    }

    #[test]
    fn same_side_orders_do_not_cause_crossing() {
        let bids_only = book_with_prices(&[90, 100], &[]);
        let asks_only = book_with_prices(&[], &[100, 110]);
        for price in [0, 100, Price::MAX] {
            assert!(!bids_only.crosses(Side::Bid, price));
            assert!(!asks_only.crosses(Side::Ask, price));
        }
    }

    #[test]
    fn bid_crosses_at_or_above_best_ask() {
        let book = book_with_prices(&[], &[100]);
        assert!(!book.crosses(Side::Bid, 99));
        assert!(book.crosses(Side::Bid, 100));
        assert!(book.crosses(Side::Bid, 101));
    }

    #[test]
    fn ask_crosses_at_or_below_best_bid() {
        let book = book_with_prices(&[100], &[]);
        assert!(book.crosses(Side::Ask, 99));
        assert!(book.crosses(Side::Ask, 100));
        assert!(!book.crosses(Side::Ask, 101));
    }

    #[test]
    fn crossing_uses_best_opposing_price_across_multiple_levels() {
        let book = book_with_prices(&[80, 90, 70], &[120, 110, 130]);
        assert!(!book.crosses(Side::Bid, 109));
        assert!(book.crosses(Side::Bid, 110));
        assert!(book.crosses(Side::Bid, 115));
        assert!(book.crosses(Side::Ask, 85));
        assert!(book.crosses(Side::Ask, 90));
        assert!(!book.crosses(Side::Ask, 91));
    }

    #[test]
    fn removing_best_ask_updates_bid_crossing() {
        let mut book = book_with_prices(&[], &[100, 110]);
        assert!(book.crosses(Side::Bid, 105));
        book.asks.remove(&100);
        assert!(!book.crosses(Side::Bid, 105));
        assert!(book.crosses(Side::Bid, 110));
        book.asks.remove(&110);
        assert!(!book.crosses(Side::Bid, Price::MAX));
    }

    #[test]
    fn removing_best_bid_updates_ask_crossing() {
        let mut book = book_with_prices(&[90, 100], &[]);
        assert!(book.crosses(Side::Ask, 95));
        book.bids.remove(&100);
        assert!(!book.crosses(Side::Ask, 95));
        assert!(book.crosses(Side::Ask, 90));
        book.bids.remove(&90);
        assert!(!book.crosses(Side::Ask, 0));
    }

    #[test]
    fn crossing_handles_price_extremes() {
        let zero_ask = book_with_prices(&[], &[0]);
        assert!(zero_ask.crosses(Side::Bid, 0));
        assert!(zero_ask.crosses(Side::Bid, Price::MAX));

        let max_ask = book_with_prices(&[], &[Price::MAX]);
        assert!(!max_ask.crosses(Side::Bid, Price::MAX - 1));
        assert!(max_ask.crosses(Side::Bid, Price::MAX));

        let zero_bid = book_with_prices(&[0], &[]);
        assert!(zero_bid.crosses(Side::Ask, 0));
        assert!(!zero_bid.crosses(Side::Ask, 1));

        let max_bid = book_with_prices(&[Price::MAX], &[]);
        assert!(max_bid.crosses(Side::Ask, 0));
        assert!(max_bid.crosses(Side::Ask, Price::MAX));
    }
}
