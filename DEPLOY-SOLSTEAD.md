# Solstead — production deploy

Deploy the game server + static site to your own VPS and domain.

## Prerequisites

- VPS with Ubuntu 22+ (2GB+ RAM recommended)
- Domain pointed at the VPS (A record → server IP)
- Node.js 20+ and npm 10+
- Rust 1.92 (`rust-toolchain.toml` in repo)
- nginx, certbot (Let's Encrypt)

## 1. Configure environment

```bash
cp .env.example .env
# Edit .env — set SOLSTEAD_DOMAIN, secrets, deploy path
```

Required production vars:

| Variable | Example |
|----------|---------|
| `SOLSTEAD_DOMAIN` | `https://solstead.xyz` |
| `AEVEN_SESSION_SIGNING_SECRET` | 32+ random bytes |
| `AEVEN_ALLOWED_ORIGINS` | `https://solstead.xyz` |
| `SITE_DEPLOY_DIR` | `/var/www/solstead` |

## 2. Build game server on VPS

```bash
cd rust-server
AEVEN_ENV=production \
AEVEN_SESSION_SIGNING_SECRET="..." \
AEVEN_ALLOWED_ORIGINS="https://solstead.xyz" \
cargo build --release --locked
```

Run as a systemd service (example):

```ini
[Unit]
Description=Solstead game server
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/solstead/rust-server
Environment=AEVEN_ENV=production
Environment=AEVEN_SESSION_SIGNING_SECRET=...
Environment=AEVEN_ALLOWED_ORIGINS=https://solstead.xyz
ExecStart=/opt/solstead/rust-server/target/release/isometric-server
Restart=always

[Install]
WantedBy=multi-user.target
```

## 3. Build and deploy site + WASM

On the VPS (or CI), from repo root:

```bash
chmod +x scripts/deploy-solstead.sh scripts/dev-browser.sh
./scripts/deploy-solstead.sh
```

This builds WASM with your domain URLs, syncs assets, runs `npm run build`, and rsyncs to `SITE_DEPLOY_DIR`.

## 4. nginx

```nginx
server {
    listen 443 ssl http2;
    server_name solstead.xyz;

    root /var/www/solstead;
    index index.html;

    location / {
        try_files $uri $uri/ $uri.html /404.html;
    }

    location ~* \.wasm$ {
        types { application/wasm wasm; }
        default_type application/wasm;
    }

    location /play/assets/ {
        add_header Cache-Control "public, max-age=31536000, immutable";
    }

    location /api/ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /matchmake/ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
    }

    location /health {
        proxy_pass http://127.0.0.1:2567;
    }

    # WebSocket: game rooms + spectator
    location ~ ^/(spectate|[0-9a-f-]{36})$ {
        proxy_pass http://127.0.0.1:2567;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
    }
}
```

```bash
sudo certbot --nginx -d solstead.xyz
sudo nginx -t && sudo systemctl reload nginx
```

## 5. Verify

```bash
curl https://solstead.xyz/health
curl -I https://solstead.xyz/play/isometric_client.wasm
curl https://solstead.xyz/api/stats/overview
```

Browser: open `https://solstead.xyz/play/` → Play as Guest or Connect Wallet (Phantom).

## Local dev

See [SETUP.md](./SETUP.md).

## Web3 (next phase)

Wallet login is live. SPL deposit/withdraw comes next — see [FORK.md](./FORK.md) Phase 1.
