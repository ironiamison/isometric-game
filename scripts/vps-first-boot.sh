#!/usr/bin/env bash
# First-boot setup for a fresh Ubuntu 22.04/24.04 VPS hosting Solstead.
# Run as root on the VPS: curl -fsSL ... | bash   OR   scp + bash scripts/vps-first-boot.sh
set -euo pipefail

DOMAIN="${1:-}"
if [ -z "$DOMAIN" ]; then
  echo "Usage: $0 yourdomain.com"
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y curl git nginx certbot python3-certbot-nginx ufw rsync build-essential pkg-config libssl-dev

# Rust (matches rust-toolchain.toml)
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.92.0
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || true
rustup target add wasm32-unknown-unknown

# Node 22 via NodeSource
if ! command -v node >/dev/null 2>&1 || [ "$(node -p "process.versions.node.split('.')[0]")" -lt 20 ]; then
  curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
  apt-get install -y nodejs
fi

mkdir -p /opt/solstead /var/www/solstead
ufw allow OpenSSH
ufw allow 'Nginx Full'
ufw --force enable

cat >/etc/nginx/sites-available/solstead <<NGINX
server {
    listen 80;
    server_name ${DOMAIN} www.${DOMAIN};

    root /var/www/solstead;
    index index.html;

    location / {
        try_files \$uri \$uri/ \$uri.html /404.html;
    }

    location ~* \\.wasm\$ {
        types { application/wasm wasm; }
        default_type application/wasm;
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
    }

    location /health {
        proxy_pass http://127.0.0.1:2567;
    }

    location /spectate {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
    }

    location ~ ^/[0-9a-f-]{36}\$ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
    }
}
NGINX

ln -sf /etc/nginx/sites-available/solstead /etc/nginx/sites-enabled/solstead
rm -f /etc/nginx/sites-enabled/default
nginx -t && systemctl reload nginx

certbot --nginx -d "$DOMAIN" -d "www.$DOMAIN" --non-interactive --agree-tos -m "admin@${DOMAIN}" || true

echo ""
echo "VPS base setup done for ${DOMAIN}"
echo "Next:"
echo "  1. Point DNS A record: ${DOMAIN} -> $(curl -s ifconfig.me 2>/dev/null || echo YOUR_VPS_IP)"
echo "  2. Clone repo to /opt/solstead and copy .env"
echo "  3. ./scripts/deploy-solstead.sh"
echo "  4. Install systemd unit for rust-server (see DEPLOY-SOLSTEAD.md)"
