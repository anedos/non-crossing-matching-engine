use std::fmt::Write;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use non_crossing_matching_engine::engine::OrderBookEngine;
use non_crossing_matching_engine::model::command::{CancelOrder, Command, NewOrder};
use non_crossing_matching_engine::model::order::{Side, Symbol, UserId, UserOrderId};

// Unique identities, 10 symbols, both sides, and 50 levels per side.
// Bids stay below asks so every input is accepted.
fn orders(count: usize) -> Vec<NewOrder> {
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

fn csv_input(count: usize) -> Vec<u8> {
    let mut input = String::new();
    for order in orders(count) {
        let side = match order.side {
            Side::Bid => "B",
            Side::Ask => "S",
        };
        writeln!(
            input,
            "N,{},{},{},{},{},{}",
            order.user_id.0, order.symbol, order.price, order.quantity, side, order.user_order_id.0,
        )
        .unwrap();
    }
    input.into_bytes()
}

// Seed 200 live orders, then cancel the oldest and insert its replacement.
// Two orders per price level keep every level populated during cancellation.
fn mixed_commands(count: usize) -> Vec<Command> {
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

fn mixed_csv_input(count: usize) -> Vec<u8> {
    let mut input = String::new();
    for command in mixed_commands(count) {
        match command {
            Command::New(order) => {
                let side = match order.side {
                    Side::Bid => "B",
                    Side::Ask => "S",
                };
                writeln!(
                    input,
                    "N,{},{},{},{},{},{}",
                    order.user_id.0,
                    order.symbol,
                    order.price,
                    order.quantity,
                    side,
                    order.user_order_id.0
                )
                .unwrap();
            }
            Command::Cancel(order) => {
                writeln!(input, "C,{},{}", order.user_id.0, order.user_order_id.0).unwrap();
            }
            Command::Flush => unreachable!(),
        }
    }
    input.into_bytes()
}

fn insert_cancel_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_cancel_events");
    for count in [1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("engine", count), &count, |b, &count| {
            b.iter_batched(
                || (OrderBookEngine::new(), mixed_commands(count)),
                |(mut engine, commands)| {
                    for command in commands {
                        let _ = black_box(engine.process(command).unwrap());
                    }
                    engine
                },
                BatchSize::LargeInput,
            );
        });
        let input = mixed_csv_input(count);
        group.bench_with_input(
            BenchmarkId::new("csv_pipeline", count),
            &input,
            |b, input| {
                b.iter_batched(
                    Vec::new,
                    |mut output| {
                        non_crossing_matching_engine::run(black_box(input.as_slice()), &mut output).unwrap();
                        output
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn order_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_events");
    for count in [1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("engine", count), &count, |b, &count| {
            b.iter_batched(
                || (OrderBookEngine::new(), orders(count)),
                |(mut engine, orders)| {
                    for order in orders {
                        let _ = black_box(engine.process(Command::New(order)).unwrap());
                    }
                    engine
                },
                BatchSize::LargeInput,
            );
        });

        let input = csv_input(count);
        group.bench_with_input(
            BenchmarkId::new("csv_pipeline", count),
            &input,
            |b, input| {
                b.iter_batched(
                    Vec::new,
                    |mut output| {
                        non_crossing_matching_engine::run(black_box(input.as_slice()), &mut output).unwrap();
                        output
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, order_events, insert_cancel_events);
criterion_main!(benches);
