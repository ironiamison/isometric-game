#!/bin/bash
# Staging deploy: builds and deploys whatever branch the staging worktree is on.
# Default branch is `refactor`. Override with STAGING_BRANCH=other ./deploy-staging.sh
set -e
source "$HOME/.cargo/env" 2>/dev/null || true

STAGING_DIR="/root/isometric-game-staging"
STAGING_BRANCH="${STAGING_BRANCH:-refactor}"
SITE_DEPLOY_DIR="${SITE_DEPLOY_DIR:-/var/www/aeven-staging}"
STAGING_SERVER_URL="${STAGING_SERVER_URL:-https://staging.aeven.xyz}"
STAGING_WS_URL="${STAGING_WS_URL:-wss://staging.aeven.xyz}"

cd "$STAGING_DIR"

# Make sure the worktree is on the desired branch, then pull.
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "$STAGING_BRANCH" ]; then
    echo "Switching staging worktree from '$CURRENT_BRANCH' to '$STAGING_BRANCH'..."
    git fetch origin "$STAGING_BRANCH"
    git checkout "$STAGING_BRANCH"
fi

BEFORE=$(git rev-parse HEAD)
git pull --ff-only origin "$STAGING_BRANCH"
AFTER=$(git rev-parse HEAD)

FORCE_ALL=0
if [ "${1:-}" = "--force" ] || [ "${FORCE_DEPLOY:-0}" = "1" ]; then
    FORCE_ALL=1
fi

if [ "$BEFORE" = "$AFTER" ] && [ "$FORCE_ALL" -eq 0 ]; then
    echo "No changes on $STAGING_BRANCH. Pass --force to redeploy anyway."
    exit 0
fi

if [ "$FORCE_ALL" -eq 1 ]; then
    CLIENT_CHANGED=1
    SERVER_CHANGED=1
    SITE_CHANGED=1
    SHARED_CHANGED=1
else
    CLIENT_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- client/ | head -1)
    SERVER_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- rust-server/ | head -1)
    SITE_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- site/ homepage/ | head -1)
    SHARED_CHANGED=$(git diff --name-only "$BEFORE" "$AFTER" -- \
        crates/ Cargo.toml Cargo.lock rust-toolchain.toml | head -1)
fi

deploy_site() {
    if ! command -v npm >/dev/null 2>&1; then
        echo "ERROR: npm not found." >&2
        exit 1
    fi

    PLAY_DIR="$STAGING_DIR/site/static/play"
    mkdir -p "$PLAY_DIR"

    echo "Building staging WASM client (SERVER_URL=$STAGING_SERVER_URL)..."
    cd "$STAGING_DIR/client"
    rustup target add wasm32-unknown-unknown 2>/dev/null || true
    AEVEN_SERVER_URL="$STAGING_SERVER_URL" \
    AEVEN_WS_URL="$STAGING_WS_URL" \
        cargo build --locked --lib --target wasm32-unknown-unknown --profile release-wasm \
            --target-dir "$STAGING_DIR/client/target"

    WASM_DIR="$STAGING_DIR/client/target/wasm32-unknown-unknown/release-wasm"
    if [ -f "$WASM_DIR/isometric_client.wasm" ]; then
        cp "$WASM_DIR/isometric_client.wasm" "$PLAY_DIR/"
    elif [ -f "$WASM_DIR/libisometric_client.wasm" ]; then
        cp "$WASM_DIR/libisometric_client.wasm" "$PLAY_DIR/isometric_client.wasm"
    elif [ -f "$WASM_DIR/isometric-client.wasm" ]; then
        cp "$WASM_DIR/isometric-client.wasm" "$PLAY_DIR/isometric_client.wasm"
    else
        echo "ERROR: No WASM artifact found in $WASM_DIR" >&2
        exit 1
    fi

    cp "$STAGING_DIR/client/web/"*.js "$PLAY_DIR/" 2>/dev/null || true
    cp "$STAGING_DIR/client/web/index.html" "$PLAY_DIR/"
    echo "Copying game assets..."
    rm -rf "$PLAY_DIR/assets"
    rsync -a "$STAGING_DIR/client/assets/" "$PLAY_DIR/assets/"

    echo "Building unified site..."
    cd "$STAGING_DIR/site"
    npm ci
    npm run build

    mkdir -p "$SITE_DEPLOY_DIR"
    rsync -av --delete "$STAGING_DIR/site/build/" "$SITE_DEPLOY_DIR/"
    echo "Site deployed to $SITE_DEPLOY_DIR"
}

if [ -n "$CLIENT_CHANGED" ] || [ -n "$SITE_CHANGED" ] || [ -n "$SHARED_CHANGED" ]; then
    deploy_site
fi

if [ -n "$SERVER_CHANGED" ] || [ -n "$SHARED_CHANGED" ]; then
    echo "Building staging server..."
    cd "$STAGING_DIR/rust-server"
    cargo build --locked --release --target-dir "$STAGING_DIR/rust-server/target"
    echo "Restarting staging server..."
    systemctl restart isometric-server-staging
    echo "Server restarted."
fi

echo "Staging deploy complete."
