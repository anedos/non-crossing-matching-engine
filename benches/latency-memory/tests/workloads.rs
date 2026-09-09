use non_crossing_matching_engine::engine::OrderBookEngine;
use non_crossing_matching_engine::model::command::Command;
use non_crossing_matching_engine::model::event::Event;
use non_crossing_matching_engine::model::order::Side;
use std::collections::HashMap;

#[path = "../../criterion/support/workloads.rs"]
mod workloads;

#[test]
fn benchmark_inputs_have_the_documented_counts_and_live_levels() {
    for size in [1_000, 10_000, 100_000] {
        for mixed in [false, true] {
            let commands = if mixed {
                workloads::mixed_commands(size)
            } else {
                workloads::orders(size)
                    .into_iter()
                    .map(Command::New)
                    .collect()
            };
            assert_eq!(commands.len(), size);
            let mut engine = OrderBookEngine::new();
            let mut live = HashMap::new();
            let mut levels = HashMap::new();
            let mut inserts = 0;
            let mut cancels = 0;
            for (position, command) in commands.into_iter().enumerate() {
                match &command {
                    Command::New(order) => {
                        let level = (order.symbol.clone(), order.side == Side::Bid, order.price);
                        assert!(live
                            .insert((order.user_id.0, order.user_order_id.0), level.clone())
                            .is_none());
                        *levels.entry(level).or_insert(0) += 1;
                        inserts += 1;
                    }
                    Command::Cancel(order) => {
                        let level = live
                            .remove(&(order.user_id.0, order.user_order_id.0))
                            .unwrap();
                        let count = levels.get_mut(&level).unwrap();
                        *count -= 1;
                        assert!(*count > 0, "mixed workload must not empty a price level");
                        cancels += 1;
                    }
                    Command::Flush => panic!("unexpected flush"),
                }
                let mut events = engine.process(command).unwrap().into_iter();
                assert!(matches!(events.next(), Some(Event::Accepted { .. })));
                assert!(events.all(|e| matches!(e, Event::TopChanged { .. })));
                if mixed && position >= 199 {
                    assert!((199..=200).contains(&live.len()));
                    assert_eq!(levels.len(), 100);
                }
            }
            if mixed {
                assert_eq!(
                    (inserts, cancels, live.len()),
                    (200 + (size - 200) / 2, (size - 200) / 2, 200)
                );
            } else {
                assert_eq!(
                    (inserts, cancels, live.len(), levels.len()),
                    (size, 0, size, 1_000)
                );
            }
        }
    }
}
