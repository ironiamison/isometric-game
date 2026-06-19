#!/usr/bin/env bash
# Verify solstead.xyz DNS points at a real VPS (not Namecheap URL forward).
set -euo pipefail

DOMAIN="${1:-solstead.xyz}"
IP=$(dig +short "$DOMAIN" A | head -1)

if [ -z "$IP" ]; then
  echo "ERROR: No A record for $DOMAIN"
  echo "Namecheap -> Advanced DNS -> A record @ -> YOUR_VPS_IP"
  exit 1
fi

echo "DNS: $DOMAIN -> $IP"

HEADERS=$(curl -sI --max-time 8 "http://$IP/" 2>/dev/null || true)
if echo "$HEADERS" | grep -qi 'namecheap'; then
  echo "ERROR: $IP is Namecheap URL Forward, not your VPS."
  echo "Remove URL Forward in Namecheap and set an A record to your VPS IP."
  exit 1
fi

if ! nc -z -w 5 "$IP" 22 2>/dev/null; then
  echo "WARN: SSH port 22 not reachable on $IP yet (firewall or server still booting)."
  exit 1
fi

echo "OK: $IP looks like a real server with SSH open."
