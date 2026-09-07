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
}