#!/usr/bin/env bash
set -euo pipefail

TARGET="armv7-unknown-linux-musleabihf"

if ! command -v zig &>/dev/null; then
    echo "Error: zig not found. Install via: mise install zig@0.15" >&2
    exit 1
fi

if ! cargo zigbuild --version &>/dev/null; then
    echo "Installing cargo-zigbuild..."
    cargo install cargo-zigbuild
fi

rustup target add "$TARGET" 2>/dev/null || true

echo "Building for $TARGET..."
cargo zigbuild --release --target "$TARGET"

echo "Done: target/$TARGET/release/romm-fuse"
