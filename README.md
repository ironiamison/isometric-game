# Aeven

An isometric pixel-art MMORPG inspired by classic RuneScape, built with Rust.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)

## About

Aeven is a multiplayer online RPG featuring grid-based movement, real-time combat, skill progression, quests, crafting, and a chunk-streamed open world — all rendered in an isometric 2.5D pixel-art style.

**Play now at [aeven.xyz](https://aeven.xyz)**

## Features

- **Multiplayer** — WebSocket-based real-time networking with server-authoritative gameplay at 20 Hz tick rate
- **Combat** — Click-to-target melee and ranged combat with hit rolls, damage numbers, and health bars
- **Skills** — RuneScape-style XP and leveling system (Hitpoints, Combat, Fishing)
- **Quests** — Lua-scripted quest system with dialogue, objectives, and rewards
- **Items & Equipment** — Inventory management, loot drops, equipment with combat bonuses
- **Shops & Crafting** — NPC merchants and recipe-based crafting
- **World** — Chunk-based world streaming with Tiled map editor support
- **NPCs** — AI state machine with idle, wander, chase, attack, and return behaviors
- **Cross-platform** — Native desktop and WASM (web) builds

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Server | Rust, Axum, Tokio, SQLite |
| Client | Rust, Macroquad |
| Protocol | MessagePack over WebSocket |
| Quest scripting | Lua |
| Game data | TOML configs |
| Map editor | React + TypeScript (Tiled-compatible) |

## Project Structure

```
client/          Rust game client (Macroquad)
rust-server/     Rust game server (Axum + WebSocket)
mapper/          React-based map editor
docs/            Design documents
```

## Building

### Server

```bash
cd rust-server
cargo run --release
```

Starts on `http://localhost:2567`.

### Client

```bash
cd client
cargo run --release
```

### WASM

```bash
cd client
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --profile release-wasm
```

## License

All rights reserved.
