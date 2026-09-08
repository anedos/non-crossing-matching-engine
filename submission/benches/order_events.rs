use std::fmt::Write;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use non_crossing_matching_engine::engine::OrderBookEngine;
use non_crossing_matching_engine::model::command::Command;
use non_crossing_matching_engine::model::order::Side;

#[path = "support/workloads.rs"]
mod workloads;
use workloads::{mixed_commands, orders};

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
