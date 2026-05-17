#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:-$HOME/.local/bin}"
mkdir -p "$TARGET_DIR"

echo "Building gaia..."
cargo build --release -p gaia-cli

install -m 755 "./target/release/gaia" "$TARGET_DIR/gaia"
echo "Installed gaia to $TARGET_DIR/gaia"
echo "Make sure $TARGET_DIR is in your PATH."
