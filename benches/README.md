# Benchmarks

There are two independent benchmarking suites:

| Suite | What it measures | Documentation |
| --- | --- | --- |
| Criterion | Average batch processing time, throughput and changes against saved baselines, for the engine and full CSV pipeline. | [Criterion README](criterion/README.md) |
| Latency and memory | Per-command P90, P95, P99 and P99.9 latency, plus separate heap allocation measurements. | [Latency and memory README](latency-memory/README.md) |

From the repository root:

```sh
./benches/criterion/run.sh
./benches/latency-memory/run.sh latency
./benches/latency-memory/run.sh memory
```

Both suites use Rust 1.92.0. The scripts print installation instructions if it is missing, without changing the default toolchain. Benchmark dependencies and instrumentation stay outside production builds.

`criterion/support` holds the workload generators and toolchain setup shared by both suites. Criterion keeps its existing reports and baselines under `target/criterion`; timestamped latency and memory results are saved under `latency-memory/results`.
