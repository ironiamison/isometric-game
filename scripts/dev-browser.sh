#!/usr/bin/env bash
# Build WASM for local browser play and refresh site/static/play/
set -euo pipefail

# Prefer rustup cargo over Homebrew (Homebrew rustc lacks wasm32 std)
export PATH="${HOME}/.cargo/bin:${PATH}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLAY_DIR="$ROOT/site/static/play"
TARGET_DIR="$ROOT/target"
WASM_DIR="$TARGET_DIR/wasm32-unknown-unknown/release-wasm"

# Talk directly to the game server (CORS allows http://localhost:5173)
export AEVEN_SERVER_URL="${AEVEN_SERVER_URL:-http://localhost:2567}"
export AEVEN_WS_URL="${AEVEN_WS_URL:-ws://localhost:2567}"
export AEVEN_ALLOW_INSECURE_ENDPOINTS=1

echo "==> Building WASM (API=$AEVEN_SERVER_URL WS=$AEVEN_WS_URL)..."
cd "$ROOT/client"
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --locked --target wasm32-unknown-unknown --profile release-wasm \
  --target-dir "$TARGET_DIR"

mkdir -p "$PLAY_DIR"
if [ -f "$WASM_DIR/isometric_client.wasm" ]; then
  cp "$WASM_DIR/isometric_client.wasm" "$PLAY_DIR/"
elif [ -f "$WASM_DIR/libisometric_client.wasm" ]; then
  cp "$WASM_DIR/libisometric_client.wasm" "$PLAY_DIR/isometric_client.wasm"
else
  echo "ERROR: WASM artifact not found in $WASM_DIR"
  exit 1
fi

cp "$ROOT/client/web/"*.js "$PLAY_DIR/"
cp "$ROOT/client/web/"*.css "$PLAY_DIR/" 2>/dev/null || true
cp "$ROOT/client/web/index.html" "$PLAY_DIR/"

echo "==> Syncing game assets (first run may take a minute)..."
rm -rf "$PLAY_DIR/assets"
rsync -a "$ROOT/client/assets/" "$PLAY_DIR/assets/"
mkdir -p "$PLAY_DIR/assets/title"
cp -R "$ROOT/client/web/assets/title/"* "$PLAY_DIR/assets/title/"

echo "==> Done. Start the game server, then: cd site && npm run dev"
echo "    Open http://localhost:5173/play/"
