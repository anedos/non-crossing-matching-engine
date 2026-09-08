# Order book engine

## Objectives
1. Correctness
2. Performance, solution is being written with performance in mind, specifically, minimising memory allocation and processing
3. Modularity, solution decouples processing from io and crate acts as both an executable and a library. This was done to aid testability for correctness and performance.

## Assumptions
1. All prices are integers, if decimals are needed the protocol needs to specify the tick size 
2. user_id and order_id is the composite key for each order, if a duplicate new order is found, the program currently halts
3. 

## Model

## Structure 
### model
Structures are declared in the model folder, with the exception of `OrderLocation` and `OrderBookEngine`, these belong to the engine.
### io
Decoupling io from the engine, allows better testability, modularity improved processing speed. 
### engine
The main processing engine is responsible for processing input and emitting the correct events.

## Performance
### Computation complexity

### Benchmarking

## Logging

Logging is disabled by default. All enabled diagnostics go to stderr; stdout is reserved
for CSV events. Set `RUST_LOG=non_crossing_matching_engine=debug` to log commands, their resulting
events, cancellations, and flushes. Use `RUST_LOG=warn` for failed commands only, or
`RUST_LOG=off` to disable logging explicitly.

The executable initializes the logger. Library callers configure their own logger.
The executable reads CSV commands from stdin and writes events to stdout.

## Output serialization

`write_event(&mut writer, &event, separator)` writes one event and a newline using
a small custom Serde serializer. The executable uses `", "` to match the exercise;
library callers can pass separators such as `"|"` or `"\t"`.

The serializer streams flat tuples of strings and unsigned integers without building
intermediate field vectors. Other Serde types return errors. Fields and separators
are written verbatim without CSV quoting or escaping; this is intended for the
exercise's fixed tokens and integer fields, not arbitrary text CSV serialization.
Buffering and the final flush belong to `run`; write and flush errors propagate to
the caller.
