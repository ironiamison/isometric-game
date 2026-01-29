#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="/root/isometric-game"
cd "$REPO_DIR"

# Get current commit before pull
BEFORE=$(git rev-parse HEAD)

# Pull latest changes
git pull origin master

# Get new commit after pull
AFTER=$(git rev-parse HEAD)

if [ "$BEFORE" = "$AFTER" ]; then
    echo "No changes, nothing to deploy."
    exit 0
fi

# Check what changed
CLIENT_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- client/ | head -1)
SERVER_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- rust-server/ | head -1)

if [ -n "$CLIENT_CHANGED" ]; then
    echo "Client changes detected, rebuilding WASM..."
    cd "$REPO_DIR/client"
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    cargo build --release --target wasm32-unknown-unknown --profile release-wasm
    # Copy WASM artifact to web directory
    cp "$REPO_DIR/client/target/wasm32-unknown-unknown/release-wasm/isometric_client.wasm" "$REPO_DIR/client/web/" 2>/dev/null || \
    cp "$REPO_DIR/client/target/wasm32-unknown-unknown/release-wasm/libisometric_client.wasm" "$REPO_DIR/client/web/isometric_client.wasm" 2>/dev/null || true
    echo "WASM build complete."
fi

if [ -n "$SERVER_CHANGED" ]; then
    echo "Server changes detected, rebuilding..."
    cd "$REPO_DIR/rust-server"
    cargo build --release
    echo "Restarting game server..."
    systemctl restart isometric-server
    echo "Server restarted."
fi

echo "Deploy complete."
