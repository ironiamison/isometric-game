# Server Performance Audit

Date: June 10, 2026

## Capacity Result

The current single-room, single-process server meets the 20 Hz tick budget in the
included 128-player release-mode simulation.

Measured on the development machine over 100 full room ticks:

- Average: 11.91 ms
- p95: 13.70 ms
- p99: 14.22 ms
- Maximum: 19.51 ms
- Tick deadline: 50 ms

The simulation uses the real world data, NPC systems, player tick path, state
serialization, compression, and 128 active per-player message queues. All players
are placed close together, which deliberately makes visibility and state sync
more expensive.

This is a strong single-process capacity signal, but it is not a substitute for
an end-to-end production test with real WebSocket clients, network latency,
authentication traffic, and concurrent database writes.

Run the capacity test with:

```sh
cargo test --release game::load_test::full_tick_stays_within_budget_for_128_players -- --ignored --nocapture
```

## Changes Made

- Moved Argon2 password hashing and verification to Tokio's blocking worker pool.
- Added SQLite WAL tuning, normal synchronous mode, foreign keys, busy timeout,
  and pool acquisition timeout.
- Serialized autosave writes instead of spawning one database task per player.
- Batched quest, recipe, spell, and chest snapshots into transactions.
- Preloaded the canonical world before accepting traffic.
- Restricted matchmaking to the canonical `game_room`, preventing arbitrary
  clients from allocating duplicate full worlds.
- Serialized room creation to prevent duplicate initialization races.
- Added expiry pruning for in-memory rate limiter entries.
- Removed several lock-across-await and inverse lock-order paths in movement,
  portals, chests, arena, KOTH, bosses, trades, NPC actions, and tick snapshots.
- Snapshot recipients before serialization and sending so sender registry locks
  are not held during CPU or queue work.
- Replaced the global player sync-state write lock with per-player DashMap entry
  locking.
- Added chunk-indexed player and NPC visibility lookup instead of scanning every
  entity for every connected player.
- Staggered periodic full state refreshes across the 20-tick window, removing the
  synchronized one-second CPU spike.

## Live Verification

A release server startup completed with:

- Canonical room preload: 131 ms
- Idle room tick p95: 1.53 ms
- Idle room tick max: 1.75 ms
- Tick overruns: 0
- `/health`: healthy
- `/api/perf`: healthy and reporting room/tick metrics
- Unknown matchmaking room: rejected with HTTP 404

The regular test suite passes with 115 tests; the release capacity test is kept
ignored during normal debug test runs.

The targeted `clippy::await_holding_lock` scan passes after suppressing one
unrelated existing deny-level lint. The repository still has substantial
pre-existing warning debt, so a global `clippy -D warnings` pass is not clean.

## Production Guardrails

Deploy the release binary only. Alert on:

- Tick loop or room tick p95 above 25 ms
- Any sustained p99 above 40 ms
- Any tick overruns
- State-sync queue skip or drop rate above 1%
- Autosave p95 above 250 ms
- Repeated SQLite busy or pool timeout errors

Use `/api/perf` for these signals. Configure the reverse proxy for WebSocket
upgrades and an idle timeout longer than the game's expected session duration.
Set appropriate process file descriptor limits for concurrent sockets.

## Remaining Limits

- SQLite remains a single-writer database. It is suitable for the current
  single-node target with bounded, serialized writes, but PostgreSQL should
  replace it before horizontal scaling or write-heavy growth.
- Game state and instances are process-local. Running multiple server replicas
  requires explicit player routing and shared coordination; adding replicas
  without that work would split the world.
- The tick scheduler still processes one canonical room. That matches the
  current global instance architecture and avoids duplicate-world corruption.
- Capacity above 128 players, mass reconnects, combat-heavy hotspots, and
  autosave under a fully populated database should be validated with external
  WebSocket load clients on production-equivalent hardware.
