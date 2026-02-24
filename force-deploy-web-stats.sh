#!/bin/bash
set -e

REPO_DIR="/root/isometric-game"
WEB_STATS_BASE_PATH="${WEB_STATS_BASE_PATH:-/players/}"
WEB_STATS_DEPLOY_DIR="${WEB_STATS_DEPLOY_DIR:-/var/www/aeven/players}"

echo "Force building and deploying web-stats..."

if ! command -v npm >/dev/null 2>&1; then
    echo "ERROR: npm not found. Install Node.js/npm on the VPS."
    exit 1
fi

cd "$REPO_DIR/web-stats"
npm ci
WEB_STATS_BASE="$WEB_STATS_BASE_PATH" npm run build

mkdir -p "$WEB_STATS_DEPLOY_DIR"
if command -v rsync >/dev/null 2>&1; then
    rsync -av --delete "$REPO_DIR/web-stats/dist/" "$WEB_STATS_DEPLOY_DIR/"
else
    rm -rf "$WEB_STATS_DEPLOY_DIR"/*
    cp -R "$REPO_DIR/web-stats/dist/." "$WEB_STATS_DEPLOY_DIR/"
fi

echo "Web stats deployed to $WEB_STATS_DEPLOY_DIR (base: $WEB_STATS_BASE_PATH)"
echo "Done."
