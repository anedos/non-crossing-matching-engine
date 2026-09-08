#!/usr/bin/env bash
set -euo pipefail

toolchain=1.92.0
bench_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v rustup >/dev/null 2>&1; then
    echo "Benchmarking requires rustup and Rust $toolchain." >&2
    echo "Please install rustup from https://rustup.rs, then run:" >&2
    echo "  rustup toolchain install $toolchain --profile minimal" >&2
    exit 1
fi

if ! compiler="$(rustup which --toolchain "$toolchain" rustc 2>/dev/null)" ||
    ! rustdoc_bin="$(rustup which --toolchain "$toolchain" rustdoc 2>/dev/null)" ||
    ! cargo_bin="$(rustup which --toolchain "$toolchain" cargo 2>/dev/null)"; then
    echo "Please download Rust $toolchain before running the benchmarks:" >&2
    echo "  rustup toolchain install $toolchain --profile minimal" >&2
    echo "Then rerun this script." >&2
    exit 1
fi

# Explicit compiler paths also work when an older Homebrew Rust is on PATH.
export RUSTC="$compiler"
export RUSTDOC="$rustdoc_bin"
exec "$cargo_bin" bench --manifest-path "$bench_dir/../Cargo.toml" --bench order_events -- "$@"
