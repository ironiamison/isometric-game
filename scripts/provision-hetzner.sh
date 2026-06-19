#!/usr/bin/env bash
# Create a Hetzner Cloud VPS for Solstead and print next steps.
#
# Prerequisites:
#   1. Hetzner Cloud account: https://console.hetzner.cloud/
#   2. API token with read/write
#   3. SSH public key uploaded to Hetzner (Project -> Security -> SSH keys)
#
# Usage:
#   export HETZNER_API_TOKEN=your_token
#   export HETZNER_SSH_KEY_NAME=solstead   # name of key in Hetzner console
#   ./scripts/provision-hetzner.sh
set -euo pipefail

: "${HETZNER_API_TOKEN:?Set HETZNER_API_TOKEN from Hetzner Cloud console}"
HETZNER_SSH_KEY_NAME="${HETZNER_SSH_KEY_NAME:-solstead}"
SERVER_TYPE="${HETZNER_SERVER_TYPE:-cx22}"
LOCATION="${HETZNER_LOCATION:-nbg1}"

api() {
  curl -fsS -H "Authorization: Bearer $HETZNER_API_TOKEN" "$@"
}

echo "==> Looking up SSH key: $HETZNER_SSH_KEY_NAME"
SSH_KEY_ID=$(api "https://api.hetzner.cloud/v1/ssh_keys" | python3 -c "
import json, sys
data = json.load(sys.stdin)
name = sys.argv[1]
for k in data.get('ssh_keys', []):
    if k['name'] == name:
        print(k['id'])
        break
" "$HETZNER_SSH_KEY_NAME")

if [ -z "$SSH_KEY_ID" ]; then
  echo "ERROR: SSH key '$HETZNER_SSH_KEY_NAME' not found in Hetzner."
  echo "Upload your public key in Hetzner Cloud -> Security -> SSH keys, then retry."
  echo ""
  echo "Generate one locally:"
  echo "  ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519_solstead -N '' -C solstead"
  echo "  cat ~/.ssh/id_ed25519_solstead.pub   # paste into Hetzner"
  exit 1
fi

echo "==> Creating server (type=$SERVER_TYPE location=$LOCATION)"
RESP=$(api -X POST "https://api.hetzner.cloud/v1/servers" \
  -H "Content-Type: application/json" \
  -d "{\"name\":\"solstead\",\"server_type\":\"$SERVER_TYPE\",\"location\":\"$LOCATION\",\"image\":\"ubuntu-24.04\",\"ssh_keys\":[$SSH_KEY_ID],\"start_after_create\":true}")

IP=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin)['server']['public_net']['ipv4']['ip'])")
ROOT_PASS=$(echo "$RESP" | python3 -c "import json,sys; print(json.load(sys.stdin).get('root_password') or '')")

echo ""
echo "VPS created."
echo "  IP: $IP"
if [ -n "$ROOT_PASS" ]; then
  echo "  Root password (save now): $ROOT_PASS"
fi
echo ""
echo "Next:"
echo "  1. Namecheap -> solstead.xyz -> remove URL Forward"
echo "     Advanced DNS -> A record @ -> $IP"
echo "     Advanced DNS -> A record www -> $IP"
echo ""
echo "  2. Wait ~2 min, then deploy:"
echo "     SOLSTEAD_SSH=root@$IP ./scripts/go-live.sh --remote"
echo ""
echo "Save this IP:"
echo "  export SOLSTEAD_VPS_IP=$IP"
