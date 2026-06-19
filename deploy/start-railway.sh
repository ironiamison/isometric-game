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

cat > /etc/nginx/conf.d/default.conf <<NGINX
server {
    listen ${PORT};
    server_name _;

    root /var/www/solstead;
    index index.html;

    client_max_body_size 20m;

    location / {
        try_files \$uri \$uri/ \$uri.html /404.html;
    }

    location ~* \\.wasm\$ {
        types { application/wasm wasm; }
        default_type application/wasm;
        add_header Cache-Control "public, max-age=3600";
    }

    location /play/assets/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
    }

    location /api/ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    location /matchmake/ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    location /health {
        proxy_pass http://127.0.0.1:2567/health;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
    }

    location /spectate {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400;
    }

    location ~ "^/[0-9a-f-]{36}\$" {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400;
    }
}
NGINX

nginx -t

echo "==> Starting nginx on port ${PORT}"
exec nginx -g 'daemon off;'
