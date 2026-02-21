#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="/root/isometric-game"
WEB_STATS_BASE_PATH="${WEB_STATS_BASE_PATH:-/players/}"
WEB_STATS_DEPLOY_DIR="${WEB_STATS_DEPLOY_DIR:-/var/www/aeven/players}"
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
WEB_STATS_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- web-stats/ | head -1)

if [ -n "$CLIENT_CHANGED" ]; then
    echo "Client changes detected, rebuilding WASM..."
    cd "$REPO_DIR/client"
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    cargo build --target wasm32-unknown-unknown --profile release-wasm
    # Copy WASM artifact to web directory
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
fi

if [ -n "$SERVER_CHANGED" ]; then
    echo "Server changes detected, rebuilding..."
    cd "$REPO_DIR/rust-server"
    cargo build --release
    echo "Restarting game server..."
    systemctl restart isometric-server
    echo "Server restarted."
fi

if [ -n "$WEB_STATS_CHANGED" ]; then
    echo "Web stats changes detected, building and deploying /players..."
    if ! command -v npm >/dev/null 2>&1; then
        echo "ERROR: npm not found. Install Node.js/npm on the VPS."
        exit 1
    fi

    cd "$REPO_DIR/web-stats"
    npm ci
    WEB_STATS_BASE="$WEB_STATS_BASE_PATH" npm run build

    mkdir -p "$WEB_STATS_DEPLOY_DIR"
    if command -v rsync >/dev/null 2>&1; then
        rsync -av --delete "$REPO_DIR/web-stats/dist/" "$WEB_STATS_DEPLOY_DIR/"
    else
        rm -rf "$WEB_STATS_DEPLOY_DIR"/*
        cp -R "$REPO_DIR/web-stats/dist/." "$WEB_STATS_DEPLOY_DIR/"
    fi
    echo "Web stats deployed to $WEB_STATS_DEPLOY_DIR (base: $WEB_STATS_BASE_PATH)"
fi

echo "Deploy complete."
