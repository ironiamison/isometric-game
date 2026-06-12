# Aeven

Aeven is a persistent isometric MMORPG with an authoritative Rust server,
desktop/browser/Android clients, data-driven gameplay, instanced spaces, live
operations tooling, and a purpose-built world/content editor.

<img width="1026" height="753" alt="Aeven gameplay" src="https://github.com/user-attachments/assets/8e0a588f-64a4-43fc-86bf-a0dfd8fbded4" />

**Play at [aeven.xyz](https://aeven.xyz).**

## Game Systems

- Persistent accounts and characters backed by SQLite
- Chunk-streamed overworld plus public and private interiors
- Server-authoritative movement, collision, combat, inventory, trading, shops,
  drops, rewards, progression, and access control
- Melee, ranged, and magic combat with equipment, prayers, spells, status
  effects, bosses, PvP, arenas, and King of the Hill
- Gathering, farming, fishing, mining, woodcutting, cooking, smithing,
  fletching, leatherworking, alchemy, Slayer, contracts, and crafting orders
- Lua-scripted quests with dialogue, objectives, rewards, and persisted state
- Banking, equipment, chests, ground items, collection logs, waystones,
  player stalls, titles, chairs, and world-map discovery
- Native desktop, browser/WASM, and Android targets
- Public world statistics and authenticated operational control pages
- React map/content studio with scoped users, validation, atomic writes, asset
  import, atlas rebuilding, and explicit deploy operations
- Self-updating desktop launcher with versioned release manifests and SHA-256
  artifact verification

## Stack

| Area | Technology |
| --- | --- |
| Authoritative server | Rust 1.92, Tokio, Axum |
| Persistence | SQLite WAL, SQLx migrations |
| Game client | Rust, Macroquad |
| Realtime protocol | Versioned MessagePack over WebSocket |
| Shared wire contract | `crates/aeven-protocol` |
| Quest scripting | Lua 5.4 through `mlua` |
| Gameplay content | TOML, Lua, versioned JSON maps |
| Mapper/content studio | React 19, TypeScript, Zustand, Express |
| Public site and control UI | SvelteKit 2, Svelte 5 |
| Launcher | Rust, eframe/egui |
| Automation | GitHub Actions, Python packaging, Cloudflare R2 |

## Repository

```text
client/                 Macroquad desktop, WASM, and Android client
crates/aeven-protocol/  Shared client-command DTOs and MessagePack codec
rust-server/            Authoritative simulation, APIs, persistence, content
mapper/                 React mapper/content studio and privileged Express API
site/                   Homepage, browser client shell, stats, control panel
launcher/               Desktop installer and updater
tools/                  Client/launcher packaging and manifest utilities
docs/                   Historical design notes; not authoritative
.github/workflows/      CI, deployment, and release automation
```

[ARCHITECTURE.md](ARCHITECTURE.md) is the authoritative system design and
[CONTRIBUTING.md](CONTRIBUTING.md) defines compatibility and quality rules.

## Prerequisites

- Rust `1.92.0` via the checked-in `rust-toolchain.toml`
- Node.js `22.x` and npm `10.x` via `.node-version` / `.nvmrc`
- Python `3.11+` for packaging and asset utilities
- Native libraries required by Macroquad and eframe on the host platform
- Android SDK/NDK only when building Android

Use committed lockfiles. Rust commands should include `--locked`; JavaScript
packages should be installed with `npm ci`.

## Run Locally

### Server

The server currently resolves `data/`, `maps/`, migrations, and the default
database relative to `rust-server/`.

```bash
cd rust-server
AEVEN_ENV=development cargo run
```

It listens on `0.0.0.0:2567` by default. Startup runs SQLx migrations, validates
the complete authoritative content graph, validates every interior, and
preloads the production room before accepting traffic.

Public endpoints include:

- `GET /health`
- `POST /api/register`
- `POST /api/login`
- `GET|POST /api/characters`
- `POST /matchmake/joinOrCreate/:room`
- `GET /api/stats/*`

`/api/perf`, `/api/logs`, and `/api/admin/*` are not mounted unless
`AEVEN_ADMIN_API_TOKEN` is configured, and then require its bearer token.

### Desktop Client

```bash
cd client
cargo run --bin new-aeven
```

Debug clients default to `http://localhost:2567` and `ws://localhost:2567`.
Release clients default to `https://aeven.xyz` and `wss://aeven.xyz`.
`AEVEN_SERVER_URL` and `AEVEN_WS_URL` override these at compile time. Release
builds reject insecure or loopback endpoints unless
`AEVEN_ALLOW_INSECURE_ENDPOINTS=1` is explicitly set for a private build.

### Mapper And Content Studio

Create `mapper/users.json` from `mapper/users.example.json`. Generate a scrypt
password hash with:

```bash
cd mapper/server
npm ci
npm run hash-password
```

Enter a password of at least 12 characters on stdin, then place the emitted hash
in `mapper/users.json`.

Run the API and frontend in separate terminals:

```bash
cd mapper/server
npm run dev
```

```bash
cd mapper
npm ci
npm run dev
```

The API binds to `127.0.0.1:3000` by default. It requires authenticated,
signed, eight-hour sessions; state-changing APIs also require a CSRF token.
Users are restricted to configured worlds. Do not expose it publicly without a
TLS reverse proxy and production secrets.

### Site

```bash
cd site
npm ci
npm run dev
```

The SvelteKit site contains the homepage, `/world/` statistics, `/control/`
operations UI, and the packaged `/play/` browser client. See
`site/DEPLOYMENT.md`.

### Launcher

```bash
cd launcher
cargo run
```

Launcher behavior is configured by `launcher/launcher-config.toml`.

## Other Client Targets

Build the WASM library:

```bash
cd client
cargo build --locked --target wasm32-unknown-unknown --profile release-wasm
```

The deploy scripts also copy `client/web/` and `client/assets/` into the
SvelteKit `/play/` output. Android integration lives in `client/android/` and
`client/src/android.rs`.

## Content

Authoritative content is server-owned:

```text
rust-server/data/             Items, entities, recipes, shops, systems
rust-server/data/quests/      Quest definitions
rust-server/data/scripts/     Lua quest scripts
rust-server/maps/world_0/     Version 2, 32x32 overworld chunks
rust-server/maps/interiors/   Public/private interior definitions
```

Startup rejects malformed files, duplicate IDs, invalid dimensions, broken
item/entity/chest/quest/interior references, bad portal destinations, invalid
loot ranges, and malformed packed collision data. The mapper keeps editable
working copies under `mapper/mapper-data/`; deploying to server content is an
explicit operation.

## Quality Gates

Run the same core checks as CI from the repository root:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo check --locked -p new-aeven-client --target wasm32-unknown-unknown --lib
cargo clippy --locked --workspace --all-targets
cargo test --locked --workspace

(cd mapper && npm ci && npm run lint && npm run build && npm audit --audit-level=moderate)
(cd mapper/server && npm ci && npm test && npm audit --audit-level=moderate)
(cd site && npm ci && npm run check && npm run build && npm audit --audit-level=moderate)
```

Server/tick changes must also pass the release-mode capacity gate:

```bash
cd rust-server
cargo test --locked --release -p isometric-server \
  full_tick_stays_within_budget_for_128_players \
  -- --ignored --nocapture
```

The current 128-player synthetic full-tick result is `p95 16.08 ms` and
`p99 17.44 ms` against a `50 ms` tick budget.

## Delivery

CI runs formatting, native/WASM checks, Clippy with warnings denied, all Rust
tests, the 128-player capacity test, frontend checks/builds/tests, and npm
audits. Production deployment and automatic client/launcher release workflows
run only after successful CI on `master`. Manual release dispatch remains
available.
