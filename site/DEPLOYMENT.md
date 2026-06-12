# Aeven Site — VPS Deployment

The `site/` package is a unified SvelteKit app that serves:

- `/` — marketing homepage
- `/world/` — world statistics (dashboard, players, leaderboards, items, bestiary)
- `/play/` — browser WASM client shell (static files copied at deploy time)
- `/control` — authenticated ops/admin panel (logs, perf, rooms, players, entities). The page is public but shows nothing without a token; its backing endpoints (`/api/perf`, `/api/logs`, `/api/admin/*`) are only registered when the Rust server is started with `AEVEN_ADMIN_API_TOKEN` set, and require that token as an `Authorization: Bearer` header.

Everything deploys to a **single nginx root** (default: `/var/www/aeven/`).

## Prerequisites on VPS

- Node.js 22.x and npm 10.x (matching `.node-version` and package metadata)
- nginx
- Rust toolchain (for WASM client builds — already required by `deploy.sh`)

## Build & deploy (automated)

`deploy.sh` handles this when `site/` changes:

1. Builds WASM → copies into `site/static/play/`
2. Copies game assets → `site/static/play/assets/`
3. Runs `npm ci && npm run build` in `site/`
4. Rsyncs `site/build/` → `/var/www/aeven/`

Force a full redeploy without waiting for git changes:

```bash
./force-deploy-site.sh
```

## nginx configuration

Use one root for the whole domain. Example:

```nginx
server {
    listen 443 ssl http2;
    server_name aeven.xyz;

    root /var/www/aeven;
    index index.html;

    # Legacy URL redirects (old /players/ links → /world/)
    location = /players {
        return 301 /world$is_args$args;
    }
    location ^~ /players/ {
        return 301 /world/$is_args$args;
    }

    # SvelteKit static output + SPA fallback for /world/* client routes
    location / {
        try_files $uri $uri/ $uri.html /404.html;
    }

    # WASM must be served with correct MIME type
    location ~* \.wasm$ {
        types { application/wasm wasm; }
        default_type application/wasm;
        add_header Cache-Control "public, max-age=3600";
    }

    # Game assets — long cache
    location /play/assets/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
        try_files $uri =404;
    }

    # API → Rust game server
    location /api/ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # WebSocket → Rust game server (adjust path if your WS endpoint differs)
    location /ws {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }
}
```

Reload nginx after changes:

```bash
sudo nginx -t && sudo systemctl reload nginx
```

## Post-deploy verification

```bash
# Homepage
curl -I https://aeven.xyz/

# World stats (live URL)
curl -I https://aeven.xyz/world/

# Legacy redirect
curl -I https://aeven.xyz/players/

# WASM MIME type
curl -I https://aeven.xyz/play/isometric_client.wasm

# Stats API (proxied to Rust server)
curl https://aeven.xyz/api/stats/overview
```

In a browser:

1. Open `https://aeven.xyz/` — homepage loads, nav links to `/world/`
2. Open `https://aeven.xyz/world/` — dashboard shows live online count
3. Open `https://aeven.xyz/play` — game canvas loads, assets have no 404s
4. Visit `https://aeven.xyz/?utm_source=test` → click Launch → URL should carry `utm_source`

## Local development

```bash
cd site
npm ci
npm run dev
```

Dev server proxies `/api` to `http://localhost:2567`. Start the Rust server locally for live stats data.

## Directory layout after deploy

```text
/var/www/aeven/
  index.html              # homepage
  homepage.css
  world/
    index.html            # stats dashboard
    players/index.html
    leaderboards/index.html
    ...
  play/
    index.html            # WASM shell
    isometric_client.wasm
    isometric_client.js
    auth.js, network.js, ...
    assets/               # sprites, atlases (large)
  screenshots/
  favicon.ico, robots.txt, sitemap.xml, ...
```

## Migrating from the old layout

Previously the VPS may have had:

- Homepage files at `/var/www/aeven/` (manual copy from `homepage/`)
- Web stats at `/var/www/aeven/world/` (from `web-stats/` build with `WEB_STATS_BASE=/world/`)
- Play client at `/var/www/aeven/play/` (from `client/web/`)

The unified `site/` build replaces all three. After first deploy, you can remove any orphaned `/var/www/aeven/players/` directory from the old `/players/` experiment.
