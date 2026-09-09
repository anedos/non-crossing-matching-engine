use non_crossing_matching_engine::model::command::{CancelOrder, Command, NewOrder};
use non_crossing_matching_engine::model::order::{Side, Symbol, UserId, UserOrderId};

// Unique identities, 10 symbols, both sides, and 50 levels per side.
// Bids stay below asks so every input is accepted.
pub fn orders(count: usize) -> Vec<NewOrder> {
    (0..count)
        .map(|i| {
            let side = if (i / 10) % 2 == 0 {
                Side::Bid
            } else {
                Side::Ask
            };
            let level = (i / 20 % 50) as u64;
            NewOrder {
                user_id: UserId((i % 100) as u64),
                user_order_id: UserOrderId(i as u64),
                symbol: Symbol::from(format!("SYM{}", i % 10)),
                side,
                price: match side {
                    Side::Bid => 1_000 - level,
                    Side::Ask => 1_100 + level,
                },
                quantity: (i % 10 + 1) as u64,
            }
        })
        .collect()
}

// Seed 200 live orders, then cancel the oldest and insert its replacement.
// Two orders per price level keep every level populated during cancellation.
pub fn mixed_commands(count: usize) -> Vec<Command> {
    const LIVE: usize = 200;
    assert!(count >= LIVE && (count - LIVE) % 2 == 0);
    let insertions = LIVE + (count - LIVE) / 2;
    let mut commands = Vec::with_capacity(count);
    for (i, mut order) in orders(insertions).into_iter().enumerate() {
        let level = (i / 20 % 5) as u64;
        order.price = match order.side {
            Side::Bid => 1_000 - level,
            Side::Ask => 1_100 + level,
        };
        if i >= LIVE {
            let cancelled = i - LIVE;
            commands.push(Command::Cancel(CancelOrder {
                user_id: UserId((cancelled % 100) as u64),
                user_order_id: UserOrderId(cancelled as u64),
            }));
        }
        commands.push(Command::New(order));
    }
    commands
}
