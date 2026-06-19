# Solstead (Aeven fork) — local setup

Fork of [andrewrexo/isometric-game](https://github.com/andrewrexo/isometric-game) for **Solstead** — isometric MMO + Solana SPL player economy. See [FORK.md](./FORK.md) for naming and on-chain roadmap.

**Upstream live demo:** [aeven.xyz](https://aeven.xyz)

## What you get

- Server-authoritative isometric MMO (combat, skills, quests, trading, banking)
- Real-time multiplayer over WebSocket (MessagePack)
- SQLite persistence (accounts, characters, world state)
- Desktop + browser (WASM) clients

## Quick start

### 1. Game server (required for online play)

```bash
cd rust-server
AEVEN_ENV=development cargo run --locked
```

Listens on **http://0.0.0.0:2567** (HTTP + WebSocket).

Health check: `curl http://localhost:2567/health`

### 2. Desktop client (easiest for local dev)

In a second terminal:

```bash
cd client
cargo run --locked --bin new-aeven
```

Debug builds auto-connect to `http://localhost:2567` and `ws://localhost:2567`.

Create an account in-game → open a second client window to test multiplayer.

### 3. Browser client

Rebuild WASM for local play (includes guest login + Solstead UI), then start the site dev server:

```bash
# Terminal 1 — game server (if not already running)
cd rust-server && AEVEN_ENV=development cargo run --locked

# Terminal 2 — build WASM + sync assets, then serve site
./scripts/dev-browser.sh
cd site && npm install --ignore-engines && npm run dev
```

Open **http://localhost:5173/play/** — click **Play as Guest**.

The bundled WASM in `site/static/play/` targets **aeven.xyz** until you run `dev-browser.sh`. Vite proxies `/api` and `/matchmake` to `:2567`; the WASM client talks to `:2567` directly for HTTP and WebSocket (CORS allows localhost dev origins).

For production deploy, set `AEVEN_SERVER_URL` / `AEVEN_WS_URL` to your domain when building WASM (see `deploy.sh`).

## Ports

| Service | Port |
|---------|------|
| Game server (HTTP + WS) | 2567 |
| Site dev (Vite) | 5173 |
| Mapper API | 3000 |

## Stack

| Layer | Tech |
|-------|------|
| Server | Rust, Tokio, Axum, SQLite |
| Client | Rust, Macroquad (native + WASM) |
| Protocol | MessagePack over WebSocket |
| Content | TOML + Lua quests + Tiled maps |

## vs SolScape / Kaetram

Aeven is a **finished MMO product** — authoritative server, content pipeline, map editor, launcher, and live ops. Much closer to Kaetram-tier completeness than SolScape.

## Next steps for your project

- Fork and rebrand (client title, site, launcher config)
- Point production deploy at your domain
- Add economy/Web3 as a thin layer on top of existing trading/GE systems
- Use `mapper/` for world/content editing
