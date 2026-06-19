#!/usr/bin/env bash
set -euo pipefail

: "${PORT:=8080}"
: "${AEVEN_SESSION_SIGNING_SECRET:?Set AEVEN_SESSION_SIGNING_SECRET (32+ chars) in Railway variables}"
: "${AEVEN_ALLOWED_ORIGINS:?Set AEVEN_ALLOWED_ORIGINS e.g. https://solstead.xyz,https://www.solstead.xyz}"

mkdir -p /data

echo "==> Starting Solstead game server on ${AEVEN_BIND_ADDR:-127.0.0.1:2567}"
cd /app/rust-server
./isometric-server &
SERVER_PID=$!

cleanup() {
  kill "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "==> Waiting for game server health"
READY=0
for i in $(seq 1 180); do
  if curl -fsS "http://127.0.0.1:2567/health" >/dev/null 2>&1; then
    echo "Game server ready"
    READY=1
    break
  fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "ERROR: game server exited during startup"
    wait "$SERVER_PID" || true
    exit 1
  fi
  sleep 2
done
if [[ "$READY" -ne 1 ]]; then
  echo "ERROR: game server did not become healthy within 6 minutes"
  exit 1
fi

echo "==> Writing nginx config for port ${PORT}"
sed "s/__PORT__/${PORT}/g" /etc/nginx/templates/default.conf.template > /etc/nginx/conf.d/default.conf

nginx -t

echo "==> Starting nginx on port ${PORT}"
exec nginx -g 'daemon off;'
