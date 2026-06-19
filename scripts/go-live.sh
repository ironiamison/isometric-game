#!/usr/bin/env bash
# Solstead go-live: build locally, optionally provision DNS + deploy to VPS.
#
# Local build only:
#   ./scripts/go-live.sh
#
# Full remote deploy (after VPS exists):
#   SOLSTEAD_SSH=root@YOUR_VPS_IP ./scripts/go-live.sh --remote
#
# Namecheap DNS (optional, needs API access):
#   NAMECHEAP_API_USER=... NAMECHEAP_API_KEY=... CLIENT_IP=$(curl -s ifconfig.me) \
#   SOLSTEAD_VPS_IP=YOUR_VPS_IP ./scripts/go-live.sh --dns
set -euo pipefail

export PATH="${HOME}/.cargo/bin:${PATH}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
DOMAIN="solstead.xyz"

ensure_env() {
  if [ ! -f "$ROOT/.env" ]; then
    SECRET=$(openssl rand -hex 32)
    cp .env.example .env
    echo "AEVEN_SESSION_SIGNING_SECRET=$SECRET" >> .env
    echo "Created .env with generated AEVEN_SESSION_SIGNING_SECRET"
  fi
  # Local builds write to repo, not /var/www
  if ! grep -q '^SITE_DEPLOY_DIR=.*/site/.production-output' .env 2>/dev/null; then
    if grep -q '^SITE_DEPLOY_DIR=' .env; then
      sed -i.bak "s|^SITE_DEPLOY_DIR=.*|SITE_DEPLOY_DIR=$ROOT/site/.production-output|" .env && rm -f .env.bak
    fi
  fi
  # shellcheck disable=SC1091
  source "$ROOT/.env"
}

build_local() {
  echo "==> Building production WASM + site"
  "$ROOT/scripts/deploy-solstead.sh"

  echo "==> Building release game server"
  cd "$ROOT/rust-server"
  AEVEN_ENV=production cargo build --release --locked
  echo "Built: $ROOT/target/release/isometric-server"
}

set_namecheap_dns() {
  : "${NAMECHEAP_API_USER:?Set NAMECHEAP_API_USER}"
  : "${NAMECHEAP_API_KEY:?Set NAMECHEAP_API_KEY}"
  : "${CLIENT_IP:?Set CLIENT_IP to your public IP (Namecheap API whitelist)}"
  : "${SOLSTEAD_VPS_IP:?Set SOLSTEAD_VPS_IP}"

  echo "==> Setting Namecheap DNS A records -> $SOLSTEAD_VPS_IP"
  curl -fsS "https://api.namecheap.com/xml.response?ApiUser=${NAMECHEAP_API_USER}&ApiKey=${NAMECHEAP_API_KEY}&UserName=${NAMECHEAP_API_USER}&ClientIp=${CLIENT_IP}&Command=namecheap.domains.dns.setHosts&SLD=solstead&TLD=xyz&HostName1=%40&RecordType1=A&Address1=${SOLSTEAD_VPS_IP}&TTL1=300&HostName2=www&RecordType2=A&Address2=${SOLSTEAD_VPS_IP}&TTL2=300" \
    | head -c 500
  echo ""
  echo "DNS update submitted. Propagation may take a few minutes."
}

remote_deploy() {
  : "${SOLSTEAD_SSH:?Set SOLSTEAD_SSH=root@YOUR_VPS_IP}"

  echo "==> Syncing repo to VPS"
  ssh -o StrictHostKeyChecking=accept-new "$SOLSTEAD_SSH" "mkdir -p /opt/solstead /var/www/solstead"
  rsync -avz --delete \
    --exclude target --exclude site/node_modules --exclude site/build --exclude .git \
    "$ROOT/" "${SOLSTEAD_SSH}:/opt/solstead/"

  echo "==> Running first-boot + deploy on VPS"
  ssh "$SOLSTEAD_SSH" bash -s "$DOMAIN" <<'REMOTE'
set -euo pipefail
DOMAIN="$1"
cd /opt/solstead
export SITE_DEPLOY_DIR=/var/www/solstead
if [ ! -f /etc/nginx/sites-enabled/solstead ]; then
  bash scripts/vps-first-boot.sh "$DOMAIN"
fi
bash scripts/deploy-solstead.sh
cd rust-server && AEVEN_ENV=production cargo build --release --locked
cd ..
cp deploy/solstead-server.service /etc/systemd/system/solstead-server.service
systemctl daemon-reload
systemctl enable solstead-server
systemctl restart solstead-server
systemctl status solstead-server --no-pager || true
REMOTE

  echo "==> Live checks"
  sleep 3
  curl -sf "https://${DOMAIN}/health" || curl -sf "http://${DOMAIN}/health" || echo "DNS/SSL may still be propagating"
  echo ""
  echo "Play: https://${DOMAIN}/play/"
}

ensure_env
build_local

for arg in "$@"; do
  case "$arg" in
    --dns) set_namecheap_dns ;;
    --remote) remote_deploy ;;
  esac
done

if [ "$#" -eq 0 ]; then
  echo ""
  echo "Local production build complete."
  echo ""
  echo "To finish go-live:"
  echo "  1. Create Ubuntu VPS (2GB+ RAM), note its IP"
  echo "  2. Namecheap -> solstead.xyz -> Advanced DNS -> A record @ and www -> VPS IP"
  echo "  3. SOLSTEAD_SSH=root@VPS_IP ./scripts/go-live.sh --remote"
  echo ""
  echo "Optional Namecheap API DNS:"
  echo "  NAMECHEAP_API_USER=... NAMECHEAP_API_KEY=... CLIENT_IP=... SOLSTEAD_VPS_IP=... ./scripts/go-live.sh --dns"
fi
