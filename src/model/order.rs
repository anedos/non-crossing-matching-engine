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

impl OrderKey {
    pub fn new(user_id: UserId, user_order_id: UserOrderId) -> Self {
        Self { user_id, user_order_id }
    }
}


// A linked-list of StoredOrder(s), previous and next point to Occupied slots only and this is
// a linked list for the same OrderBook, side and PriceLevel.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct StoredOrder {
    pub quantity: Quantity,
    pub previous: Option<OrderIndex>,
    pub next: Option<OrderIndex>,
}

impl StoredOrder {
    pub fn new(quantity: Quantity, previous: Option<OrderIndex>, next: Option<OrderIndex>) -> Self {
        Self { quantity, previous, next }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum OrderSlot {
    Occupied(StoredOrder),
    Free {
        next_free: Option<OrderIndex>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Orders {
    slots: Vec<OrderSlot>,
    first_free: Option<OrderIndex>,
    #[cfg(test)]
    fail_next_allocation: bool,
}

impl Orders {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            first_free: None,
            #[cfg(test)]
            fail_next_allocation: false,
        }
    }

    /// Access an active order. An invalid index indicates inconsistent engine state.
    pub fn get(&self, idx: OrderIndex) -> &StoredOrder {
        match self.slots.get(idx.0 as usize).expect("order index is out of bounds") {
            OrderSlot::Occupied(order) => order,
            OrderSlot::Free { .. } => panic!("active order index points to a free slot"),
        }
    }

    fn get_mut(&mut self, idx: OrderIndex) -> &mut StoredOrder {
        match self.slots.get_mut(idx.0 as usize).expect("order index is out of bounds") {
            OrderSlot::Occupied(order) => order,
            OrderSlot::Free { .. } => panic!("active order index points to a free slot"),
        }
    }

    /// Unlink an active order and push its slot onto the free-list head.
    pub fn remove(&mut self, idx: OrderIndex) -> StoredOrder {
        let order = *self.get(idx);
        // Check both neighbors before changing either link.
        if let Some(previous) = order.previous {
            assert_eq!(self.get(previous).next, Some(idx), "previous order has an inconsistent next link");
        }
        if let Some(next) = order.next {
            assert_eq!(self.get(next).previous, Some(idx), "next order has an inconsistent previous link");
        }

        if let Some(previous) = order.previous {
            self.get_mut(previous).next = order.next;
        }
        if let Some(next) = order.next {
            self.get_mut(next).previous = order.previous;
        }
        self.slots[idx.0 as usize] = OrderSlot::Free { next_free: self.first_free };
        self.first_free = Some(idx);
        order
    }

    /// Reset storage while retaining the allocated slot capacity.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.first_free = None;
        #[cfg(test)]
        {
            self.fail_next_allocation = false;
        }
    }

    // create a new StoredOrder, set the previous point to the PriceLevel's last
    // and the previous order's next to the new StoredOrder.
    pub fn append_after(
        &mut self,
        previous: Option<OrderIndex>,
        quantity: Quantity
    ) -> Result<OrderIndex, std::num::TryFromIntError>{
        let order = StoredOrder::new(quantity, previous, None);
        let idx = self.add_order(order)?;

        if let Some(previous_idx) = previous {
            match &mut self.slots[previous_idx.0 as usize] {
                OrderSlot::Occupied(previous_order) => {
                    previous_order.next = Some(idx);
                }
                OrderSlot::Free { .. } => {
                    panic!("price level's last order points to a free slot");
                }
            }
        }

        Ok(idx)
    }

    fn add_order(&mut self, order: StoredOrder) -> Result<OrderIndex, std::num::TryFromIntError> {
        // Test the capacity-error path without allocating billions of slots.
        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_allocation) {
            return u32::try_from(u64::MAX).map(OrderIndex);
        }

        if let Some(idx) = self.first_free {
            let slot = &mut self.slots[idx.0 as usize];

            self.first_free = match slot {
                OrderSlot::Free {
                    next_free,
                } => *next_free,
                OrderSlot::Occupied(_) => {
                    panic!("first_free points to an occupied slot");
                }
            };
            *slot = OrderSlot::Occupied(order);
            Ok(idx)
        } else {
            let idx: OrderIndex = OrderIndex(u32::try_from(self.slots.len())?);
            self.slots.push(OrderSlot::Occupied(order));
            Ok(idx)
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_next_allocation(&mut self) {
        self.fail_next_allocation = true;
    }

    #[cfg(test)]
    pub(crate) fn inspect_order(&self, idx: OrderIndex) -> StoredOrder {
        match &self.slots[idx.0 as usize] {
            OrderSlot::Occupied(order) => *order,
            OrderSlot::Free { .. } => panic!("expected an occupied order slot"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_retains_capacity_and_resets_free_list() {
        crate::test_logging::init_test_logging();
        let mut orders = Orders::new();
        let first = orders.append_after(None, 2).unwrap();
        let second = orders.append_after(Some(first), 3).unwrap();
        orders.remove(first);
        let capacity = orders.slots.capacity();
        assert_eq!(orders.first_free, Some(first));
        assert_eq!(*orders.get(second), StoredOrder::new(3, None, None));

        orders.clear();
        assert!(orders.slots.is_empty());
        assert_eq!(orders.first_free, None);
        assert_eq!(orders.slots.capacity(), capacity);
        let fresh = orders.append_after(None, 5).unwrap();
        assert_eq!(fresh, OrderIndex(0));
        assert_eq!(*orders.get(fresh), StoredOrder::new(5, None, None));
    }
}
