#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="/root/isometric-game"
cd "$REPO_DIR"

# Build client WASM
echo "Building WASM client..."
cd "$REPO_DIR/client"
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --target wasm32-unknown-unknown --profile release-wasm
WASM_DIR="$REPO_DIR/client/target/wasm32-unknown-unknown/release-wasm"
if [ -f "$WASM_DIR/isometric_client.wasm" ]; then
    cp "$WASM_DIR/isometric_client.wasm" "$REPO_DIR/client/web/"
    echo "Copied isometric_client.wasm"
elif [ -f "$WASM_DIR/libisometric_client.wasm" ]; then
    cp "$WASM_DIR/libisometric_client.wasm" "$REPO_DIR/client/web/isometric_client.wasm"
    echo "Copied libisometric_client.wasm"
elif [ -f "$WASM_DIR/isometric-client.wasm" ]; then
    cp "$WASM_DIR/isometric-client.wasm" "$REPO_DIR/client/web/isometric_client.wasm"
    echo "Copied isometric-client.wasm"
else
    echo "ERROR: No WASM artifact found in $WASM_DIR"
    ls -la "$WASM_DIR"/*.wasm 2>/dev/null || echo "No .wasm files found"
    exit 1
fi
echo "WASM build complete."

# Build and restart server
echo "Building server..."
cd "$REPO_DIR/rust-server"
cargo build --release
echo "Restarting game server..."
systemctl restart isometric-server
echo "Server restarted."

echo "Force deploy complete."
