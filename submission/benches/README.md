Run locally from the repository root:

For per-command P90/P95/P99/P99.9 and heap allocation measurements, see the separate [latency and memory benchmark](metrics/README.md). The commands below run Criterion throughput benchmarks.

```sh
./submission/benches/run.sh
```

The script uses Rust 1.92.0 specifically for benchmarking because Criterion 0.8 requires a newer compiler than the non-crossing matching engine assessment. It leaves your default toolchain unchanged. If rustup or Rust 1.92.0 is missing, it exits with installation instructions; it does not download anything automatically. The script works from any working directory and forwards arguments to Criterion. Run it without arguments for a normal benchmark run.

Criterion is an independent package under `benches/criterion`, so its dependencies are not resolved by production builds or engine tests. The script keeps using `submission/target` for artifacts and existing saved baselines. Run `./submission/benches/run.sh --test` for benchmark smoke checks; `cargo test --manifest-path submission/Cargo.toml` runs engine and I/O tests. Host-only benchmarks are excluded from the Docker build context.

The engine now uses `ahash::AHashMap`. To compare it against an existing standard-hasher baseline on the same machine and toolchain:

```sh
./submission/benches/run.sh --baseline std-hashmap
```

For a fresh comparison, temporarily replace `use ahash::AHashMap as HashMap;` in `src/engine/mod.rs` with `use std::collections::HashMap;`, run `./submission/benches/run.sh --save-baseline std-hashmap`, restore the ahash import, and run the comparison command above. Baseline files are local and ignored by Git. `--save-baseline` records whichever implementation is currently compiled; its name does not select the hasher.

Criterion measures 1,000, 10,000, and 100,000 input commands per iteration and reports time per batch and commands per second. Both deterministic workloads use unique insertion identities, 10 symbols, and both sides. All insertions are accepted and every cancellation targets a live order. Neither workload covers rejections or flush commands.

- `order_events`: insertions only, with 50 price levels per side per symbol.
- `insert_cancel_events`: 200 initial insertions, then alternating cancellation of the oldest order and insertion of a replacement. There are five price levels per side per symbol, initially with two orders each, so every level remains populated. The live order count alternates between 199 and 200; replacements can reuse freed slots. Initial insertions are included in timing and throughput.

| Mixed batch size | Insertions | Cancellations |
| ---: | ---: | ---: |
| 1,000 | 600 | 400 |
| 10,000 | 5,100 | 4,900 |
| 100,000 | 50,100 | 49,900 |

To run only the mixed workload:

```sh
./submission/benches/run.sh insert_cancel_events
```

To run only the four 100,000-command cases:

```sh
./submission/benches/run.sh '/100000$'
```

The `engine` benchmark starts with an empty engine and excludes engine construction and input generation from timing. Processing includes map growth and order storage allocations; final engine destruction is excluded. The `csv_pipeline` benchmark includes parsing, engine processing, serialization into a growing memory buffer, flushing, and engine destruction. Input generation and final output-buffer destruction are excluded. Neither benchmark measures disk I/O or process startup.

Use an idle machine for comparisons. Cargo builds benchmarks with the optimized bench profile. Reports and saved baselines are under `submission/target/criterion/`; the HTML overview is `submission/target/criterion/report/index.html`.

Initial local baseline (2026-09-08, Rust 1.92.0, aarch64-apple-darwin, standard HashMap hasher, Criterion 0.8.2 defaults: 100 samples, 3-second warm-up, 5-second target measurement):

| Benchmark | Input orders | Time per batch (95% confidence interval) | Orders/second (point estimate) |
| --- | ---: | ---: | ---: |
| Engine | 1,000 | 206.23–208.33 µs | 4.83 million |
| Engine | 10,000 | 1.8977–1.9055 ms | 5.26 million |
| CSV pipeline | 1,000 | 923.17–929.77 µs | 1.08 million |
| CSV pipeline | 10,000 | 8.8474–8.8942 ms | 1.13 million |

The mixed workload exercises reuse of freed order slots, but a timing comparison with insertions alone does not isolate allocation savings: command mix, live book size, and number of price levels also differ. These benchmarks do not count allocations.

Combined run after adding cancellations (same date, toolchain, and configuration; saved as `std-hashmap`, replacing the initial baseline):

| Benchmark | Commands | Insertions only | Insertions + cancellations |
| --- | ---: | ---: | ---: |
| Engine | 1,000 | 215.63 µs | 151.45 µs |
| Engine | 10,000 | 1.9451 ms | 1.4899 ms |
| CSV pipeline | 1,000 | 944.25 µs | 789.06 µs |
| CSV pipeline | 10,000 | 8.8312 ms | 7.6105 ms |

Values are Criterion point estimates per batch. Mixed-workload 95% confidence intervals were 150.72–152.41 µs and 1.4754–1.5095 ms for the engine, and 787.22–790.87 µs and 7.5945–7.6269 ms for the CSV pipeline. The unchanged insertion-only engine's 10,000-command result moved by roughly 2% from the first run, so small differences should be interpreted cautiously.

The 100,000-command cases were run separately with the same toolchain and configuration and added to `std-hashmap`:

| Benchmark | Insertions only | Insertions + cancellations |
| --- | ---: | ---: |
| Engine | 21.108 ms | 14.676 ms |
| CSV pipeline | 93.875 ms | 77.546 ms |

The 95% confidence intervals were 20.249–22.202 ms and 14.640–14.713 ms for the engine, and 92.550–95.500 ms and 77.182–78.004 ms for the CSV pipeline. Criterion extended measurement beyond the five-second target to collect all 100 samples for the CSV cases.


Ahash comparison (Rust 1.92.0, same workloads and Criterion settings, against the saved `std-hashmap` baseline):

| Workload | Commands | Benchmark | Standard (ms) | Ahash (ms) | Time reduction |
| --- | ---: | --- | ---: | ---: | ---: |
| Insertions | 1,000 | engine | 0.2156 | 0.1191 | 43.3% |
| Insertions | 1,000 | csv_pipeline | 0.9442 | 0.8655 | 8.4% |
| Insertions | 10,000 | engine | 1.9451 | 1.0455 | 46.1% |
| Insertions | 10,000 | csv_pipeline | 8.8312 | 8.0469 | 8.9% |
| Insertions | 100,000 | engine | 21.1079 | 10.4127 | 50.7% |
| Insertions | 100,000 | csv_pipeline | 93.8750 | 83.6360 | 10.9% |
| Insertions + cancellations | 1,000 | engine | 0.1515 | 0.0808 | 47.5% |
| Insertions + cancellations | 1,000 | csv_pipeline | 0.7891 | 0.7308 | 7.6% |
| Insertions + cancellations | 10,000 | engine | 1.4899 | 0.8181 | 45.5% |
| Insertions + cancellations | 10,000 | csv_pipeline | 7.6105 | 7.3601 | 3.3% |
| Insertions + cancellations | 100,000 | engine | 14.6760 | 8.4461 | 42.4% |
| Insertions + cancellations | 100,000 | csv_pipeline | 77.5457 | 73.9257 | 4.7% |

All 12 cases were classified as improved by Criterion (p < 0.05). Time reduction uses Criterion's mean-change estimate; displayed batch times use its usual slope estimate where available, otherwise the mean, so percentages can differ from ratios of the displayed times. Engine time fell by 42–51%; CSV pipeline time fell by 3–11%. These are local measurements against earlier runs, not a simultaneous comparison. All 36 correctness tests passed with ahash. The engine now retains the ahash implementation. The standard baseline is unchanged; the comparison samples are in each case's `new/` directory and its statistics in `change/`.
