#!/usr/bin/env bash
set -euo pipefail

bench_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

source "$bench_dir/support/toolchain.sh"
exec "$cargo_bin" bench --manifest-path "$bench_dir/../Cargo.toml" --bench order_events -- "$@"
