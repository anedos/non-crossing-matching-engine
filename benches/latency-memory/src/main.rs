use std::hint::black_box;

use non_crossing_matching_engine::engine::OrderBookEngine;
use non_crossing_matching_engine::model::command::Command;

#[path = "../../criterion/support/workloads.rs"]
mod workloads;

fn commands(count: usize, mixed: bool) -> Vec<Command> {
    if mixed {
        workloads::mixed_commands(count)
    } else {
        workloads::orders(count)
            .into_iter()
            .map(Command::New)
            .collect()
    }
}

#[cfg(not(feature = "memory"))]
fn histogram() -> hdrhistogram::Histogram<u64> {
    hdrhistogram::Histogram::new(3).unwrap()
}

#[cfg(not(feature = "memory"))]
fn print_latency(workload: &str, size: usize, operation: &str, h: &hdrhistogram::Histogram<u64>) {
    println!(
        "{workload},{size},{operation},{},{},{},{},{}",
        h.len(),
        h.value_at_quantile(0.90),
        h.value_at_quantile(0.95),
        h.value_at_quantile(0.99),
        h.value_at_quantile(0.999)
    );
}

#[cfg(not(feature = "memory"))]
fn main() {
    use std::time::Instant;
    // Every operation histogram receives at least this many observations.
    const SAMPLES: usize = 1_000_000;
    println!("workload,batch_commands,operation,samples,p90_ns,p95_ns,p99_ns,p99_9_ns");
    let mut timer = histogram();
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let elapsed = start.elapsed();
        timer
            .record(elapsed.as_nanos().try_into().unwrap())
            .unwrap();
    }
    print_latency("timer_overhead", 0, "empty", &timer);

    for mixed in [false, true] {
        let name = if mixed { "mixed" } else { "insertions" };
        for size in [1_000, 10_000, 100_000] {
            // Warm up the code and allocator using a disposable engine.
            let mut warm = OrderBookEngine::new();
            for command in commands(size, mixed) {
                let _ = black_box(warm.process(command).unwrap());
            }
            drop(warm);

            let mut insert = histogram();
            let mut cancel = histogram();
            while insert.len() < SAMPLES as u64 || (mixed && cancel.len() < SAMPLES as u64) {
                let input = commands(size, mixed);
                let mut engine = OrderBookEngine::new();
                for command in input {
                    let is_cancel = matches!(command, Command::Cancel(_));
                    let start = Instant::now();
                    let result = black_box(engine.process(command));
                    let elapsed = start.elapsed();
                    let _ = result.unwrap();
                    let h = if is_cancel { &mut cancel } else { &mut insert };
                    h.record(elapsed.as_nanos().try_into().unwrap()).unwrap();
                }
            }
            print_latency(name, size, "insert", &insert);
            if mixed {
                print_latency(name, size, "cancel", &cancel);
            }
        }
    }
}

#[cfg(feature = "memory")]
fn copy_command(command: &Command) -> Command {
    use non_crossing_matching_engine::model::command::{CancelOrder, NewOrder};
    match command {
        Command::New(order) => Command::New(NewOrder {
            user_id: order.user_id,
            user_order_id: order.user_order_id,
            symbol: order.symbol.clone(),
            side: order.side,
            price: order.price,
            quantity: order.quantity,
        }),
        Command::Cancel(order) => Command::Cancel(CancelOrder {
            user_id: order.user_id,
            user_order_id: order.user_order_id,
        }),
        Command::Flush => Command::Flush,
    }
}

#[cfg(feature = "memory")]
fn main() {
    println!("workload,batch_commands,allocations,total_bytes,peak_live_bytes,retained_bytes,after_drop_bytes");
    // Initialize process-wide hasher/runtime state before measuring engine storage.
    let mut warm = OrderBookEngine::new();
    for command in commands(1_000, true) {
        let _ = black_box(warm.process(command).unwrap());
    }
    drop(warm);
    for mixed in [false, true] {
        let name = if mixed { "mixed" } else { "insertions" };
        for size in [1_000, 10_000, 100_000] {
            // Original Rc symbols stay alive throughout processing AND engine drop.
            // Cloning commands therefore does not allocate or free input storage.
            let input = commands(size, mixed);
            let mut engine = None;
            let stats = allocation_counter::measure(|| {
                let current = engine.insert(OrderBookEngine::new());
                for command in &input {
                    let _ = black_box(current.process(copy_command(command)).unwrap());
                }
            });
            let cleanup = allocation_counter::measure(|| drop(engine.take()));
            let remaining = stats.bytes_current + cleanup.bytes_current;
            assert_eq!(remaining, 0, "engine allocations must be released on drop");
            assert_eq!(stats.count_current + cleanup.count_current, 0);
            println!(
                "{name},{size},{},{},{},{},{remaining}",
                stats.count_total, stats.bytes_total, stats.bytes_max, stats.bytes_current
            );
        }
    }
}
