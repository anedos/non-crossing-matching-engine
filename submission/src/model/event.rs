use super::order::{UserId, UserOrderId, Side};
use super::book::{TopOfBook};

pub enum Event {
    Accepted {
        user_id: UserId,
        user_order_id: UserOrderId,
    },
    Rejected {
        user_id: UserId,
        user_order_id: UserOrderId,
    },
    TopChanged {
        side: Side,
        top: Option<TopOfBook>,
    },
}

#[must_use]
pub enum Events {
    None,
    One(Event),
    Two(Event, Event),
}
