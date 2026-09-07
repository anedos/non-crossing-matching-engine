# Order book engine

## Objectives

## Assumptions
1. All prices are integers, if decimals are needed the protocol needs to specify the tick size 
2. user_id and order_id is the composite key for each order, if a duplicate new order is found, the program currently halts
3. 

## Model

## Structure

## Performance

## Logging

Logging is disabled by default. All enabled diagnostics go to stderr; stdout is reserved
for CSV events. Set `RUST_LOG=non_crossing_matching_engine=debug` to log commands, their resulting
events, cancellations, and flushes. Use `RUST_LOG=warn` for failed commands only, or
`RUST_LOG=off` to disable logging explicitly.

The executable initializes the logger. Library callers configure their own logger.
The executable's CSV input/output wiring is still unfinished, so enabling logging does
not yet make the executable process orders.
