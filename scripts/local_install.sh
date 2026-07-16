#!/bin/bash
set -euo pipefail

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/romm-fuse"

echo "=== romm-fuse installer ==="

# Check dependencies
if ! command -v fusermount3 &>/dev/null && ! command -v fusermount &>/dev/null; then
    echo "Error: fusermount not found. Install fuse3:"
    echo "  Debian/Ubuntu: sudo apt install fuse3 libfuse3-dev"
    echo "  Arch: sudo pacman -S fuse3"
    exit 1
fi

# Build
echo "Building romm-fuse..."
cargo build --release

# Install binary
echo "Installing binary to ${INSTALL_DIR}/romm-fuse"
sudo install -m 755 target/release/romm-fuse "${INSTALL_DIR}/romm-fuse"

# Install config directory
echo "Creating config directory ${CONFIG_DIR}"
sudo mkdir -p "${CONFIG_DIR}"
if [ ! -f "${CONFIG_DIR}/config.toml" ]; then
    sudo cp examples/config.toml "${CONFIG_DIR}/config.toml"
    echo "Created default config at ${CONFIG_DIR}/config.toml"
    echo "  -> Edit this file with your RomM URL and API token"
else
    echo "Config already exists at ${CONFIG_DIR}/config.toml, skipping"
fi

# Install systemd service (if systemd is available)
if command -v systemctl &>/dev/null; then
    echo "Installing systemd service"
    sudo cp examples/systemd/romm-fuse.service /etc/systemd/system/
    sudo systemctl daemon-reload
    echo "  -> systemctl enable --now romm-fuse"
fi

echo ""
echo "Done! Quick start:"
echo "  1. Edit ${CONFIG_DIR}/config.toml with your RomM URL and token"
echo "  2. Create mountpoint: mkdir -p /media/fat/games"
echo "  3. Mount: romm-fuse /media/fat/games"
echo ""
echo "For systemd: systemctl enable --now romm-fuse"
