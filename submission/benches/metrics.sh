#!/usr/bin/env bash
set -euo pipefail
bench_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ $# -ne 1 || ( "$1" != latency && "$1" != memory ) ]]; then
    echo "Usage: $0 latency|memory" >&2
    exit 1
fi
source "$bench_dir/support/toolchain.sh"
features=()
if [[ "$1" == memory ]]; then
    features=(--features memory)
fi
results_dir="$bench_dir/metrics/results"
mkdir -p "$results_dir"
run_stamp="$(date -u +%Y-%m-%dT%H-%M-%SZ)"
measurement=latency-percentiles-ns
if [[ "$1" == memory ]]; then
    measurement=heap-allocations-bytes
fi
output_base="$results_dir/${run_stamp}_engine_insertions-and-cancellations_${measurement}_$$"
if "$cargo_bin" run --release --locked --manifest-path "$bench_dir/metrics/Cargo.toml" "${features[@]}" \
    > "$output_base.csv" 2> "$output_base.log"; then
    cat "$output_base.csv"
    printf 'Results saved to %s.csv\nExecution log: %s.log\n' "$output_base" "$output_base" >&2
else
    run_status=$?
    mv "$output_base.csv" "$output_base.failed.csv"
    cat "$output_base.log" >&2
    printf 'Benchmark failed; partial output: %s.failed.csv\n' "$output_base" >&2
    exit "$run_status"
fi
