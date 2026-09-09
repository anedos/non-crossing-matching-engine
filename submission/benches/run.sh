#!/usr/bin/env bash
set -euo pipefail

bench_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

source "$bench_dir/support/toolchain.sh"
exec "$cargo_bin" bench --locked --manifest-path "$bench_dir/criterion/Cargo.toml" --target-dir "$bench_dir/../target" --bench order_events -- "$@"
