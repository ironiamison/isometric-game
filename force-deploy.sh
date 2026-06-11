#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true

REPO_DIR="/root/isometric-game"
SITE_DEPLOY_DIR="${SITE_DEPLOY_DIR:-/var/www/aeven}"
cd "$REPO_DIR"

# Build and restart server
echo "Building server..."
cd "$REPO_DIR/rust-server"
cargo build --release
echo "Restarting game server..."
systemctl restart isometric-server
echo "Server restarted."

# Delegate site deploy to force-deploy-site.sh
"$REPO_DIR/force-deploy-site.sh"

echo "Force deploy complete."
