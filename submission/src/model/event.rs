use super::order::{UserId, UserOrderId, Side};
use super::book::{TopOfBook};

#[derive(Debug)]
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
#[derive(Debug)]
pub enum Events {
    None,
    One(Event),
    Two(Event, Event),
}

impl IntoIterator for Events {
    type Item = Event;
    type IntoIter = std::iter::Flatten<std::array::IntoIter<Option<Event>, 2>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::None => [None, None],
            Self::One(event) => [Some(event), None],
            Self::Two(first, second) => [Some(first), Some(second)],
        }.into_iter().flatten()
    }
}
