# Engine latency and heap allocation benchmarks

Run from the repository root on the host, using the same Rust 1.92.0 toolchain as Criterion:

```sh
./submission/benches/metrics.sh latency
./submission/benches/metrics.sh memory
```

The script prints installation instructions if rustup or Rust 1.92.0 is missing. It never installs a toolchain or changes your default compiler. Both modes build optimized executables. Each run automatically saves CSV results and an execution log under `submission/benches/metrics/results/`, independent of your working directory. Nothing is written to the repository's `reports/` directory.

Filenames include the UTC start timestamp, workload, measurement and units, and process ID to distinguish runs started in the same second. For example:

```sh
2026-09-08T11-01-40Z_engine_insertions-and-cancellations_latency-percentiles-ns_12345.csv
2026-09-08T11-01-40Z_engine_insertions-and-cancellations_heap-allocations-bytes_12346.csv
```

Successful results are also printed as CSV on stdout; saved paths are printed on stderr. Cargo diagnostics are preserved in the matching `.log` file. Failed runs return a nonzero status and label partial output `.failed.csv`. Saved results are ignored by Git. The earlier timestamped ahash results have also been moved into this directory, with their original names preserved.

## Production isolation

This directory is an independent, unpublished Cargo package that depends on the engine library by path. Neither its dependencies nor its features are added to the submission package. No instrumentation is added to `submission/src`.

- Latency mode uses `hdrhistogram` and the normal allocator.
- Memory mode enables this package's optional `memory` feature, linking `allocation-counter` and its instrumented allocator into this executable only. It reports no latency numbers.
- Production builds and Criterion do not link either metrics crate. Running memory mode does not affect subsequent production or latency builds: Cargo builds each dependency/feature combination separately.

The two modes are deliberately separate builds so allocation tracking does not distort the latency measurements. Build artifacts are ignored under `metrics/target`; this package's lockfile is checked in for reproducible dependency versions.

## Workloads and latency interpretation

The generators in `../support/workloads.rs` are shared with Criterion. Both modes run batches of 1,000, 10,000, and 100,000 commands:

- `insertions`: an initially empty engine grows to the batch size, across 10 symbols, both sides, and 50 levels per side per symbol.
- `mixed`: 200 seed insertions followed by alternating cancellation and replacement. The engine keeps 199–200 live orders and five levels per side per symbol. Seed insertions are included. This does not benchmark a book with 100,000 live orders in the mixed case.

Latency mode warms up each scenario, then repeats entire batches with fresh engines until **each reported operation histogram has at least 1,000,000 observations**. Insertions and cancellations have separate histograms. Each row includes its actual sample count; rounding to complete batches can exceed the minimum. There are nine operation rows and one empty-timer row.

Each timed interval covers one `engine.process(command)` call and a result optimization barrier. Command generation, engine construction/destruction, result checking, histogram recording, and printing occur outside the interval. Engine allocations and deallocations performed by `process` are included. Input parsing and output serialization are not measured.

`p90_ns`, `p95_ns`, `p99_ns`, and `p99_9_ns` are latency percentiles in **nanoseconds per command**, not batch times. For example, P99 = 300 ns means approximately 99% of observations were at or below 300 ns. Histograms use three significant digits, so values are quantized. P99.9 has about 1,000 observations above it at the minimum sample count, but this is not a confidence guarantee: repeated operations can be correlated.

The `timer_overhead` row measures an empty `Instant::now()` / `elapsed()` interval. Its values are reported separately and are **not subtracted** from operation percentiles. At these short durations, clock overhead and resolution matter. Histogram updates are outside timing but can still affect caches between calls. OS scheduling pauses remain in the samples; nothing trims outliers. Run on an idle machine and repeat runs before drawing conclusions about tails.

This is sequential in-process service time. It does not include queueing, concurrent arrivals, network latency, or throughput under load. It is not an end-to-end production latency SLA, nor a substitute for Criterion's statistical comparison of throughput.

## Memory interpretation

Memory mode warms up runtime/hasher initialization, then measures one fresh-engine batch per row. It retains the prepared input commands outside the measured region and copies their scalar fields and `Rc` symbol references without heap allocation. Keeping the originals alive through engine destruction prevents input deallocations from contaminating the counters.

| Column | Meaning |
| --- | --- |
| `allocations` | Allocation calls reported by the counter during engine construction and processing. |
| `total_bytes` | Cumulative bytes allocated, including storage subsequently freed. |
| `peak_live_bytes` | Maximum simultaneously live tracked bytes during the batch. |
| `retained_bytes` | Tracked bytes still live with the populated engine retained, before drop. |
| `after_drop_bytes` | Net tracked bytes after dropping the engine; asserted to be zero. |

These are **additional requested heap bytes for engine storage**, not total process RAM/RSS. Input vector storage and symbol strings, histogram storage, stack memory, allocator bookkeeping, and process-wide initialization are excluded. Symbol allocations are excluded even where the engine retains a shared reference. Engine map capacity, price-level storage, and order slots are included. Retained capacity is legitimate memory use, not a leak.

The mixed workload exercises freed-slot reuse. Compare allocation counts and bytes across batch sizes: a bounded live book should avoid the storage growth seen with insertion-only batches. Differences also reflect the smaller live book and different number of levels; these measurements do not isolate the linked-list design from all other factors. They report memory for whole workloads, not memory percentiles per event.

## Dependencies

- [hdrhistogram](https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html): duration histograms and quantiles.
- [allocation-counter](https://docs.rs/allocation-counter/latest/allocation_counter/): thread-local allocation accounting through an instrumented global allocator. These workloads are single-threaded.

The existing [Criterion benchmarks](../README.md) remain the tool for average throughput and baseline comparisons.
