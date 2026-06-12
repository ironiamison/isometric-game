#!/bin/bash
set -euo pipefail
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="${REPO_DIR:-/root/isometric-game}"
SITE_DEPLOY_DIR="${SITE_DEPLOY_DIR:-/var/www/aeven}"

echo "Force building and deploying unified site..."

if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    echo "ERROR: Node.js 22 and npm 10 are required."
    exit 1
fi
NODE_MAJOR=$(node -p "process.versions.node.split('.')[0]")
NPM_MAJOR=$(npm --version | cut -d. -f1)
if [ "$NODE_MAJOR" != "22" ] || [ "$NPM_MAJOR" != "10" ]; then
    echo "ERROR: Node.js 22.x and npm 10.x are required; found node $(node --version), npm $(npm --version)."
    exit 1
fi

# Build WASM client
echo "Building WASM client..."
cd "$REPO_DIR/client"
rustup target add wasm32-unknown-unknown 2>/dev/null || true
AEVEN_SERVER_URL="${AEVEN_SERVER_URL:-https://aeven.xyz}" \
AEVEN_WS_URL="${AEVEN_WS_URL:-wss://aeven.xyz}" \
    cargo build --locked --target wasm32-unknown-unknown --profile release-wasm \
        --target-dir "$REPO_DIR/client/target"
WASM_DIR="$REPO_DIR/client/target/wasm32-unknown-unknown/release-wasm"
PLAY_DIR="$REPO_DIR/site/static/play"
mkdir -p "$PLAY_DIR"

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

# Build SvelteKit site
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
echo "Done."
