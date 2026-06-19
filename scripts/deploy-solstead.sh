#!/usr/bin/env bash
# Build and deploy Solstead site + WASM to a VPS.
# Usage: copy .env.example to .env, fill in values, then: ./scripts/deploy-solstead.sh
set -euo pipefail

export PATH="${HOME}/.cargo/bin:${PATH}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ -f "$ROOT/.env" ]; then
  # shellcheck disable=SC1091
  source "$ROOT/.env"
fi

: "${SOLSTEAD_DOMAIN:?Set SOLSTEAD_DOMAIN in .env (e.g. https://solstead.xyz)}"
: "${SITE_DEPLOY_DIR:?Set SITE_DEPLOY_DIR in .env (e.g. /var/www/solstead)}"

# Strip trailing slash
SOLSTEAD_DOMAIN="${SOLSTEAD_DOMAIN%/}"
WS_URL="${AEVEN_WS_URL:-${SOLSTEAD_DOMAIN/http/ws}}"
HTTP_URL="${AEVEN_SERVER_URL:-$SOLSTEAD_DOMAIN}"

echo "==> Building WASM for $HTTP_URL"
export AEVEN_SERVER_URL="$HTTP_URL"
export AEVEN_WS_URL="$WS_URL"
export AEVEN_ALLOW_INSECURE_ENDPOINTS=1
export VITE_SITE_URL="$SOLSTEAD_DOMAIN"

"$ROOT/scripts/dev-browser.sh"

echo "==> Building SvelteKit site"
cd "$ROOT/site"
npm ci --ignore-engines 2>/dev/null || npm install --ignore-engines
VITE_SITE_URL="$SOLSTEAD_DOMAIN" npm run build

echo "==> Deploying to $SITE_DEPLOY_DIR"
mkdir -p "$SITE_DEPLOY_DIR"
if command -v rsync >/dev/null 2>&1; then
  rsync -av --delete "$ROOT/site/build/" "$SITE_DEPLOY_DIR/"
else
  rm -rf "${SITE_DEPLOY_DIR:?}/"*
  cp -R "$ROOT/site/build/." "$SITE_DEPLOY_DIR/"
fi

echo "==> Done. Ensure nginx proxies /api, /matchmake, /health, /spectate, and WS room paths to :2567"
echo "    Site root: $SITE_DEPLOY_DIR"
echo "    Public URL: $SOLSTEAD_DOMAIN"
