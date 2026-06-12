#!/bin/bash
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="${REPO_DIR:-/root/isometric-game}"
SITE_DEPLOY_DIR="${SITE_DEPLOY_DIR:-/var/www/aeven}"
cd "$REPO_DIR"

require_node_toolchain() {
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
        echo "ERROR: Node.js 22 and npm 10 are required."
        exit 1
    fi

    local node_major npm_major
    node_major=$(node -p "process.versions.node.split('.')[0]")
    npm_major=$(npm --version | cut -d. -f1)
    if [ "$node_major" != "22" ] || [ "$npm_major" != "10" ]; then
        echo "ERROR: Node.js 22.x and npm 10.x are required; found node $(node --version), npm $(npm --version)."
        exit 1
    fi
}

# Get current commit before pull
BEFORE=$(git rev-parse HEAD)

# Pull latest changes
git pull --ff-only origin master

# Get new commit after pull
AFTER=$(git rev-parse HEAD)

if [ "$BEFORE" = "$AFTER" ]; then
    echo "No changes, nothing to deploy."
    exit 0
fi

# Check what changed
CLIENT_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- client/ | head -1)
SERVER_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- rust-server/ | head -1)
SITE_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- site/ | head -1)
SHARED_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- \
    crates/aeven-protocol/ Cargo.toml Cargo.lock rust-toolchain.toml | head -1)

deploy_site() {
    require_node_toolchain

    local PLAY_DIR WASM_DIR
    PLAY_DIR="$REPO_DIR/site/static/play"
    mkdir -p "$PLAY_DIR"

    echo "Building WASM client..."
    cd "$REPO_DIR/client"
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    AEVEN_SERVER_URL="${AEVEN_SERVER_URL:-https://aeven.xyz}" \
    AEVEN_WS_URL="${AEVEN_WS_URL:-wss://aeven.xyz}" \
        cargo build --locked --target wasm32-unknown-unknown --profile release-wasm \
            --target-dir "$REPO_DIR/client/target"
    WASM_DIR="$REPO_DIR/client/target/wasm32-unknown-unknown/release-wasm"
    if [ -f "$WASM_DIR/isometric_client.wasm" ]; then
        cp "$WASM_DIR/isometric_client.wasm" "$PLAY_DIR/"
    elif [ -f "$WASM_DIR/libisometric_client.wasm" ]; then
        cp "$WASM_DIR/libisometric_client.wasm" "$PLAY_DIR/isometric_client.wasm"
    elif [ -f "$WASM_DIR/isometric-client.wasm" ]; then
        cp "$WASM_DIR/isometric-client.wasm" "$PLAY_DIR/isometric_client.wasm"
    else
        echo "ERROR: No WASM artifact found in $WASM_DIR"
        exit 1
    fi

    cp "$REPO_DIR/client/web/"*.js "$PLAY_DIR/" 2>/dev/null || true
    cp "$REPO_DIR/client/web/index.html" "$PLAY_DIR/"
    echo "Copying game assets..."
    rm -rf "$PLAY_DIR/assets"
    rsync -a "$REPO_DIR/client/assets/" "$PLAY_DIR/assets/"

    echo "Building unified site..."
    cd "$REPO_DIR/site"
    npm ci
    npm run build

    mkdir -p "$SITE_DEPLOY_DIR"
    if command -v rsync >/dev/null 2>&1; then
        rsync -av --delete "$REPO_DIR/site/build/" "$SITE_DEPLOY_DIR/"
    else
        rm -rf "$SITE_DEPLOY_DIR"/*
        cp -R "$REPO_DIR/site/build/." "$SITE_DEPLOY_DIR/"
    fi
    echo "Site deployed to $SITE_DEPLOY_DIR"
}

if [ -n "$CLIENT_CHANGED" ] || [ -n "$SITE_CHANGED" ] || [ -n "$SHARED_CHANGED" ]; then
    deploy_site
fi

if [ -n "$SERVER_CHANGED" ] || [ -n "$SHARED_CHANGED" ]; then
    echo "Server changes detected, rebuilding..."
    cd "$REPO_DIR/rust-server"
    cargo build --locked --release --target-dir "$REPO_DIR/rust-server/target"
    echo "Restarting game server..."
    systemctl restart isometric-server
    echo "Server restarted."
fi

echo "Deploy complete."
