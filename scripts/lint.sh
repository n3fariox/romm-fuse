#!/usr/bin/env bash
set -euo pipefail

echo "=== format ==="
cargo fmt --all

echo "=== clippy ==="
cargo clippy --workspace --all-targets -- -D warnings

echo "=== done ==="
