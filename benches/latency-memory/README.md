# Engine latency and heap allocation benchmarks

## Results at a glance

Snapshot of the latest saved successful runs: **2026-09-09 UTC**. These are historical
measurements, not a fresh benchmark of the current checkout. The saved logs do not
record the host hardware or source commit. This section is a static snapshot; running
the scripts saves new CSV files but does not update these tables automatically.

### Engine latency

Values are **nanoseconds per command**. Each operation row contains at least 1,000,000
samples. The empty-timer baseline was **42 ns** at all four percentiles and has not
been subtracted. These timings exclude CSV parsing and output serialization.

| Workload | Batch commands | Operation | P90 | P95 | P99 | P99.9 |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Insertions | 1,000 | Insert | 125 | 167 | 250 | 5,503 |
| Insertions | 10,000 | Insert | 125 | 166 | 208 | 750 |
| Insertions | 100,000 | Insert | 125 | 166 | 208 | 375 |
| Mixed | 1,000 | Insert | 125 | 125 | 250 | 1,375 |
| Mixed | 1,000 | Cancel | 84 | 125 | 125 | 167 |
| Mixed | 10,000 | Insert | 125 | 125 | 166 | 583 |
| Mixed | 10,000 | Cancel | 125 | 125 | 125 | 167 |
| Mixed | 100,000 | Insert | 84 | 125 | 125 | 250 |
| Mixed | 100,000 | Cancel | 84 | 125 | 125 | 167 |

### Engine heap allocations

Each row measures one **whole batch**, not an individual command. **Total bytes
allocated** is cumulative: it includes allocations that were subsequently freed or
replaced as storage grew. **Peak live bytes** is the maximum simultaneously live
tracked storage; **retained bytes** is what remains at the end, before engine drop.
These counters exclude prepared input storage and symbol strings and do not measure
process RAM/RSS.

Insertion-only batches retain every inserted order, so larger batches grow the live
book and require more allocation. Mixed batches start with 200 insertions, then
alternate cancellation and replacement, keeping 199–200 live orders after seeding.
A 100,000-command mixed batch therefore contains 50,100 insertions and 49,900
cancellations and ends with 200 live orders. A bounded live book alone does not
guarantee constant cumulative allocation; the saved mixed results below show that
storage reuse keeps allocation counts and bytes unchanged across these batch sizes.

| Workload | Batch commands | Allocations | Total bytes allocated | Peak live bytes | Retained bytes | Bytes after drop |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Insertions | 1,000 | 182 | 312,288 | 229,608 | 186,784 | 0 |
| Insertions | 10,000 | 189 | 2,454,520 | 1,462,696 | 1,257,888 | 0 |
| Insertions | 100,000 | 195 | 19,198,992 | 11,268,520 | 9,630,112 | 0 |
| Mixed | 1,000 | 38 | 71,440 | 52,200 | 39,648 | 0 |
| Mixed | 10,000 | 38 | 71,440 | 52,200 | 39,648 | 0 |
| Mixed | 100,000 | 38 | 71,440 | 52,200 | 39,648 | 0 |

In this snapshot, the mixed workload's allocation counts and bytes stay constant as
the command count increases. Insertion-only storage grows with the live book. Latency
tails vary across scenarios; repeat runs on an idle machine before comparing them.

Local source files: [latency CSV](results/2026-09-09T12-36-21Z_engine_insertions-and-cancellations_latency-percentiles-ns_85470.csv)
and [allocation CSV](results/2026-09-09T12-36-20Z_engine_insertions-and-cancellations_heap-allocations-bytes_85471.csv).
These raw files are ignored by Git, so the links require the original local results.
The methodology and measurement limits are documented below.

## Running the benchmarks

Run from the repository root on the host, using the same Rust 1.92.0 toolchain as Criterion:

```sh
./benches/latency-memory/run.sh latency
./benches/latency-memory/run.sh memory
```

The script prints installation instructions if rustup or Rust 1.92.0 is missing. It never installs a toolchain or changes your default compiler. Both modes build optimized executables. Each run automatically saves CSV results and an execution log under `benches/latency-memory/results/`, independent of your working directory. Nothing is written to the repository's `reports/` directory.

Filenames include the UTC start timestamp, workload, measurement and units, and process ID to distinguish runs started in the same second. For example:

```sh
2026-09-08T11-01-40Z_engine_insertions-and-cancellations_latency-percentiles-ns_12345.csv
2026-09-08T11-01-40Z_engine_insertions-and-cancellations_heap-allocations-bytes_12346.csv
```

Successful results are also printed as CSV on stdout; saved paths are printed on stderr. Cargo diagnostics are preserved in the matching `.log` file. Failed runs return a nonzero status and label partial output `.failed.csv`. Saved results are ignored by Git. The earlier timestamped ahash results have also been moved into this directory, with their original names preserved.

## Production isolation

This directory is an independent, unpublished Cargo package that depends on the engine library by path. Neither its dependencies nor its features are added to the main package. No instrumentation is added to `src`.

- Latency mode uses `hdrhistogram` and the normal allocator.
- Memory mode enables this package's optional `memory` feature, linking `allocation-counter` and its instrumented allocator into this executable only. It reports no latency numbers.
- Production builds and Criterion do not link either metrics crate. Running memory mode does not affect subsequent production or latency builds: Cargo builds each dependency/feature combination separately.

The two modes are deliberately separate builds so allocation tracking does not distort the latency measurements. Build artifacts are ignored under `latency-memory/target`; this package's lockfile is checked in for reproducible dependency versions.

## Workloads and latency interpretation

The generators in `../criterion/support/workloads.rs` are shared with Criterion. Both modes run batches of 1,000, 10,000, and 100,000 commands:

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

The existing [Criterion benchmarks](../criterion/README.md) remain the tool for average throughput and baseline comparisons.

The shared workload invariants (command counts, acceptance, live-order bound, and populated levels) can be checked with:

```sh
rustup run 1.92.0 cargo test --manifest-path benches/latency-memory/Cargo.toml --locked
```

If your environment overrides the compiler, also set `RUSTC="$(rustup which --toolchain 1.92.0 rustc)"` for that command, as the benchmark scripts do.
