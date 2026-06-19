# Solstead on Railway

One Railway service runs **nginx** (static site + `/play` WASM) and the **Rust game server** behind it.

## 1. Create the service

1. [Railway dashboard](https://railway.app/) → **New Project** → **Deploy from GitHub repo**
2. Connect **`ironiamison/isometric-game`** (branch `master`)
3. Railway detects `Dockerfile` + `railway.toml` automatically

First deploy will fail until required variables are set (step 2).

## 2. Variables (Railway → Service → Variables)

### Required (runtime)

| Variable | Example |
|----------|---------|
| `AEVEN_SESSION_SIGNING_SECRET` | `openssl rand -hex 32` |
| `AEVEN_ALLOWED_ORIGINS` | `https://solstead.xyz,https://www.solstead.xyz` |

Optional:

| Variable | Default |
|----------|---------|
| `AEVEN_DATABASE_URL` | `sqlite:/data/game.db?mode=rwc` |
| `AEVEN_ADMIN_API_TOKEN` | (unset — admin API off) |

### Build-time (for WASM URLs baked into the client)

Add these as **build variables** (Railway → Variables → mark as available during build):

| Variable | Value |
|----------|-------|
| `PUBLIC_URL` | `https://solstead.xyz` |
| `AEVEN_WS_URL` | `wss://solstead.xyz` |

If you test on `*.up.railway.app` first, set `PUBLIC_URL` / `AEVEN_WS_URL` to that HTTPS URL, then **redeploy** when you switch to `solstead.xyz`.

Also add the Railway URL to `AEVEN_ALLOWED_ORIGINS` while testing:

```
https://your-app.up.railway.app,https://solstead.xyz,https://www.solstead.xyz
```

## 3. Persistent volume (SQLite)

Railway → Service → **Volumes** → Add volume:

- **Mount path:** `/data`

Without this, player/world data resets on every deploy.

## 4. Custom domain (`solstead.xyz`)

1. Railway → Service → **Settings** → **Networking** → **Custom Domain** → add `solstead.xyz` and `www.solstead.xyz`
2. Railway shows CNAME/A targets — add them in **Namecheap → Advanced DNS**
3. **Remove Namecheap URL Forward** (currently pointing at a parking IP)
4. Set build vars `PUBLIC_URL` / `AEVEN_WS_URL` to `https://solstead.xyz` / `wss://solstead.xyz`
5. **Redeploy** so WASM is rebuilt for the final domain

## 5. Verify

```bash
curl https://solstead.xyz/health
curl https://solstead.xyz/api/stats/overview
curl -I https://solstead.xyz/play/isometric_client.wasm
```

Browser: https://solstead.xyz/play/ → Play as Guest

## Notes

- **Build time:** ~15–25 min first deploy (Rust + WASM + site). Later deploys use Docker layer cache.
- **Cost:** One service + volume; scale to 2GB+ RAM if tick budget tests fail under load.
- **Updates:** Push to `master` → Railway auto-redeploys.

## Local Docker smoke test

```bash
docker build -t solstead --build-arg PUBLIC_URL=https://solstead.xyz .
docker run --rm -p 8080:8080 \
  -e AEVEN_SESSION_SIGNING_SECRET="$(openssl rand -hex 32)" \
  -e AEVEN_ALLOWED_ORIGINS=https://solstead.xyz \
  -e PORT=8080 \
  -v solstead-data:/data \
  solstead
# open http://localhost:8080/play/
```
