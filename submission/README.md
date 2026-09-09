# Order book engine

The system presented here is a non-crossing order engine as per the specification in [EXERCISE.md](../EXERCISE.md). This
is a single-threaded order engine optimized for fast processing and memory efficiency. It has minimal dependencies, unit
and integration tests and two different benchmarking suites, one based
on [Criterion](https://github.com/bheisler/criterion.rs)
and a bespoke one that measures latency percentiles (P90, P95, P99 and P99.9) and heap allocation. No middleware
currently exists between order input, processing and output. However, due to the engine's modularity, a message bus
could be added to decouple order processing from I/O. There is no order matching done by the engine as it wasn't asked
for by the specification.

## Objectives

1. Correctness, strict adherence to the specification given in the exercise. Where ambiguity or gaps exist, sound and
   explicit assumptions have been made, documented below.
2. Performance, the engine has been written with performance in mind, specifically, minimising memory allocation and
   processing overhead.
3. Modularity, the engine decouples processing from I/O and the crate acts as both an executable and a library. This was
   done to aid testability for correctness and performance.

## Assumptions

1. Prices and quantities are unsigned 64-bit integers. If decimals are needed, the protocol needs to specify the scale
   or tick size. Zero prices and quantities are currently accepted; a zero-quantity order still occupies its price level
   until canceled.
2. `userId` and `userOrderId` form the composite key for each order, across all symbols. If a duplicate new order is
   found, the engine returns an error and the executable halts. The same `userOrderId` can be used by different users.
3. Cancelling an unknown or already canceled order also returns an error. Malformed input and price-level quantity
   overflow stop the executable rather than being treated as crossing rejections.
4. Crossing orders emit a rejection and leave the book unchanged; processing continues. Their identity is not reserved.
   Identities can also be reused after cancellation or a flush.
5. A flush clears all books without emitting events. It retains the order-slot vector and hash-map capacities for reuse,
   but drops the price-level trees.

## Model

Each symbol has an `OrderBook`, with separate `BTreeMap`s for bids and asks. A `PriceLevel` holds the aggregate quantity
and the first and last order indices. Orders at the same price form a doubly linked FIFO list, using indices into a
shared `Vec` rather than individually allocated list nodes.

The engine maps each order's composite key to an `OrderLocation`, which holds its symbol, side, price and slot index.
This allows cancellation to find and unlink an order without scanning the level. Canceled slots form a free list and
are reused by subsequent insertions. Slot indices are 32-bit; exhausting that index space returns an error.

### Order insertion

For a valid, non-crossing order, the new slot joins the end of its price level's FIFO list.
Duplicate identities, crossing orders and quantity overflow are checked before changing the model.

```mermaid
flowchart LR
    A[New order] --> B[Find OrderBook by symbol and PriceLevel by side and price]
    B --> C[Validate identity, crossing and total quantity]
    C --> D[Orders: reuse a free slot or append to the vector]
    D --> E[Link the previous tail to the new slot]
    E --> F[Create book or level if needed; update first, last and total quantity]
    F --> G[Store OrderLocation under userId + userOrderId]
    G --> H[Emit Accepted, then TopChanged if the top changed]
```

### Order cancellation

The order's location gives us its slot and price level directly. Its neighbours are linked together,
and the freed slot becomes available for the next insertion.

```mermaid
flowchart LR
    A[Cancel order] --> B[Find OrderLocation by userId + userOrderId]
    B --> C[Locate OrderBook, PriceLevel and order slot]
    C --> D[Orders: unlink neighbours and put the slot on the free list]
    D --> E[PriceLevel: update first, last and total quantity]
    E --> F[Remove empty price level; remove book if both sides are empty]
    F --> G[Remove OrderLocation]
    G --> H[Emit Accepted, then TopChanged if the top changed]
```

An unknown order returns an error before changing the model.

## Structure

### model

Structures are declared in the model folder, with the exception of `OrderLocation` and `OrderBookEngine`, which belong
to the engine.

### io

Decoupling I/O from the engine allows better testability and modularity. This module parses CSV commands and serializes
output events. The library's `run(reader, writer)` connects these to the engine, so the writer can be stdout, a file or
an in-memory buffer.

### engine

The main processing engine is responsible for processing commands and emitting the correct events. It does not read
input or write output itself.

### benches

The engine's benchmarking suites. Additional info in the [benchmark README](benches/README.md).

## Build and tests

From the repository root:

```sh
cargo build --release --manifest-path submission/Cargo.toml
cargo test --manifest-path submission/Cargo.toml
```

The supplied non-crossing matching engine Docker build and all 12 stdin test cases have also passed with Rust 1.75 on Linux x64:

```sh
docker build --platform linux/amd64 -t non-crossing-matching-engine .
docker run --rm --platform linux/amd64 non-crossing-matching-engine
```

Tests cover the supplied fixtures, malformed input, I/O failures and engine edge cases. A separate test compares 8,000
mixed commands against a simple reference model, including FIFO links and aggregate quantities.

## Performance

The engine now uses `ahash::AHashMap` for additional performance. I have benchmarked standard `HashMap` against
`AHashMap` and
`AHashMap` reduced engine processing time by up to ~50% in these workloads. The full CSV pipeline improved by roughly
3–11%; these are local measurements, not a guarantee for every workload.

### Computational complexity

The engine has been written with explicit data structures and operations that can be analysed easily. Here, N is the
number of price levels in a symbol's book.

- Order addition is O (log N) on average, with expected constant-time hash-map lookups and constant-time FIFO linking.
  Growing the hash maps or order vector can make an individual insertion more expensive; their insertion cost is
  amortised.
- Order removal is also O (log N) on average. Finding the order uses a hash map, unlinking it is constant time, and
  accessing or removing its price level uses the tree.
- Flushing is linear to stored state: it clears order locations and drops books and their price levels. Its cost 
  depends on the stored state and the hash-map capacities visited while clearing.

Memory usage depends on the order storage, price levels and symbols. Cancellation reuses slots but does not shrink the
allocated order vector. Its capacity therefore follows the high-water mark rather than just the number of live orders.

### Benchmarking

See the [Criterion README](benches/README.md) and the [latency and memory README](benches/metrics/README.md). Both
suites run on the host with Rust 1.92 and are separate Cargo packages, keeping their dependencies and instrumentation
out of production builds. The scripts explain how to install the toolchain if it is missing.

## Logging

Logging is disabled by default. All enabled diagnostics go to stderr; stdout is reserved for CSV events. Set
`RUST_LOG=non_crossing_matching_engine=debug` to log commands, their resulting events, cancellations, and flushes. Use
`RUST_LOG=warn` for engine warnings, or
`RUST_LOG=off` to disable logging explicitly. Fatal errors are still reported on stderr by the executable.

Unit and integration tests also support optional logging, through a shared `init_test_logging` helper.
From the repository root:

```sh
RUST_LOG=non_crossing_matching_engine=debug cargo test --manifest-path submission/Cargo.toml -- --nocapture
```

Without `RUST_LOG`, test logging stays off. Add a test-name filter before `--` to inspect a particular test,
or `--test-threads=1` after it to avoid interleaved output. The subprocess tests for production logging
set their own filters so they can verify that logging is silent by default and stays on stderr.

## Output serialization

`write_event(&mut writer, &event, separator)` writes one event and a newline using a small custom Serde serializer. The
executable uses `", "` to match the exercise; library callers can pass separators such as `"|"` or `"\t"`.

The serializer streams flat tuples of strings and unsigned integers without building intermediate field vectors. Other
Serde types return errors. Fields and separators are written verbatim without CSV quoting or escaping; this is intended
for the exercise's fixed tokens and integer fields, not arbitrary text CSV serialization. Buffering and the final flush
belong to `run`; write and flush errors propagate to the caller.

## Future work

I would love to turn this into a matching engine that executes crossing orders, with no memory allocations during
processing within a preallocated capacity. Growing beyond that capacity would need an explicit allocation or
capacity-limit policy. Order processing would remain strictly single-threaded on a per-symbol basis, with a thread
pinned to a core. I would also introduce generic I/O middleware so orders can be read and events written outside of the
main processing thread.

Introduce an `EventSink` abstraction, in the form of a trait, so that output is abstracted and a message bus can be
easily integrated, e.g., Redis, Kafka or Aeron. This would also need explicit handling of backpressure, delivery
failures and event routing by symbol.

A more elaborate benchmarking suite covering cases such as millions of orders and thousands of symbols.
