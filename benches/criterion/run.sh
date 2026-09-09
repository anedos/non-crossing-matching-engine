#!/usr/bin/env bash
set -euo pipefail

bench_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

source "$bench_dir/support/toolchain.sh"
export CRITERION_HOME="$(cd "$bench_dir/../.." && pwd)/target/criterion"
exec "$cargo_bin" bench --locked --manifest-path "$bench_dir/Cargo.toml" --target-dir "$bench_dir/../../target" --bench order_events -- "$@"
