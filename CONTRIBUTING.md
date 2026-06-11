# Contributing To Aeven

This repository contains a live multiplayer game, content pipeline, web tools,
and release system. A change that looks local can affect the wire protocol,
persistent characters, map data, platform builds, or production operations.

Read [ARCHITECTURE.md](ARCHITECTURE.md) before making structural changes.

## Development Setup

Required tools:

- Rust 1.88 or newer
- Node.js 20.19 or newer and npm
- Python 3.11 or newer for packaging work
- Native libraries required by Macroquad or eframe on your platform

Install dependencies in each JavaScript package you intend to change:

```bash
(cd mapper && npm install)
(cd mapper/server && npm install)
(cd web-stats && npm install)
```

Rust dependencies are resolved by Cargo in `client/`, `rust-server/`, and
`launcher/`.

Run the game server from `rust-server/` because its data, map, and database
paths are currently relative to that working directory:

```bash
cd rust-server
cargo run
```

Run the desktop client separately:

```bash
cd client
cargo run --bin new-aeven
```

## Before You Change Code

Identify the boundaries affected by the change:

- Does it change a client command or server event?
- Does it alter persisted state or database schema?
- Does it add a gameplay system or another lock to `GameRoom`?
- Does it affect desktop, WASM, or Android differently?
- Does it change a content ID referenced by maps, loot, quests, or saves?
- Does it expose data through HTTP, WebSocket, logs, metrics, or the mapper API?
- Does it affect packaging, release manifests, or endpoint configuration?

Prefer a focused change with explicit compatibility handling over a broad
cleanup mixed with gameplay behavior.

## Engineering Rules

### Server Authority

Clients send intent. The server derives outcomes.

Every mutating command must validate the relevant subset of:

- Authenticated account and active character
- Current command lease and connection generation
- Entity, item, object, or activity ownership
- Shared instance or world context
- Position, proximity, collision, and line of sight
- Cooldown, action state, and activity state
- Quantity, price, inventory capacity, and resource availability
- Skill, quest, equipment, and progression requirements
- Duplicate, replayed, stale, and out-of-order commands

Never accept client-calculated damage, rewards, XP, prices, drops, positions,
completion state, or target access.

### Domain Ownership

`GameRoom` and client `GameState` are already too broad. Do not add unrelated
state to them merely because they are globally reachable.

New server systems should normally have:

- A type that owns the system's mutable state
- Command methods with validated inputs
- Narrow read-only queries
- Domain events for cross-system and protocol output
- A defined tick phase, if ticking is required
- A persistence snapshot or change set, if durable

Avoid new `use super::*` imports. Import the types a module actually depends on.
Do not expose locks as a module API; expose operations.

On the client, separate authoritative replicated data from local presentation,
input, and UI state. A server message handler should update the smallest
relevant state slice rather than know about every screen.

### Async And Locking

- Do not hold a lock across `.await` unless the protected invariant explicitly
  requires it and the critical section is bounded.
- Establish and document lock order when one operation needs multiple locks.
- Copy or snapshot small values before asynchronous I/O.
- Use bounded channels for per-connection work.
- Treat dropped private state and failed bootstrap messages as correctness
  failures, not silent best-effort work.
- Keep expensive filesystem, database, compression, and parsing work off the
  latency-sensitive tick path.

### Errors And Logging

Classify failures:

- **Fatal startup:** schema mismatch, missing required maps/content, invalid
  production configuration
- **Command error:** invalid or unauthorized player intent
- **Connection error:** malformed protocol, timeout, backpressure, send failure
- **Best effort:** optional telemetry or cleanup that does not affect truth

Do not discard `Result` values with `.ok()` or `let _ =` unless the operation is
genuinely best effort and the reason is documented. Logs must not contain
passwords, bearer tokens, signed session IDs, private chat, or unnecessary
personal data.

### Configuration And Secrets

- Do not commit credentials, tokens, local user databases, `.env` files, or
  production host secrets.
- Do not add another hard-coded service endpoint.
- Keep local, staging, and production configuration explicit.
- Production releases must reject localhost game endpoints.
- The mapper API is privileged and must remain local-only until its mutation
  routes are properly authenticated and protected.

## Protocol Changes

The protocol is manually duplicated today, so changes require extra care.

For a client-to-server command:

1. Update `client/src/network/messages.rs`.
2. Update `rust-server/src/protocol/decode.rs`.
3. Add server dispatch and domain validation.
4. Add tests for valid, malformed, unauthorized, stale, and cross-instance
   inputs.

For a server-to-client event:

1. Update the server message type and encoder.
2. Choose unicast, spatial/instance scope, or global broadcast deliberately.
3. Update client dispatch and the owning client state module.
4. Test missing fields, old fields, and unknown message types.

Keep message names and field casing stable. If compatibility cannot be
preserved, introduce an explicit protocol version and document the minimum
client/server versions. Do not reuse an old message name for different
semantics.

The intended destination is a shared wire-only Cargo crate. Do not move server
domain objects into that crate; shared DTOs are contracts, not game ownership.

## Database And Persistence Changes

Persistent data is a compatibility boundary. Test both:

- A fresh database created from the current code
- An existing database upgraded from the previous released schema

Until versioned migrations are introduced:

- Keep schema updates idempotent.
- Fail on unexpected migration errors.
- Do not use broad error suppression to handle duplicate columns or tables.
- Keep related player state changes in one transaction.
- Preserve old serialized fields with Serde defaults or an explicit migration.
- Consider rollback and partial-write behavior.

Prefer a typed snapshot or repository API to adding more positional parameters
to `save_character`.

Never edit a production SQLite database manually as part of a normal deploy.

## Content And Map Changes

Authoritative content is under `rust-server/data/` and `rust-server/maps/`.
The mapper may use `mapper-data/` as editable working state.

For content changes:

1. Keep released IDs stable.
2. Use the content studio where a structured editor exists.
3. Validate references to items, entities, drops, recipes, shops, quests,
   skills, objects, assets, chunks, and interiors.
4. Start the server and confirm every required registry loads.
5. Exercise the affected content in a client when practical.
6. Review generated TOML, JSON, and atlas diffs for unrelated churn.

For map changes:

- Preserve the 32x32 overworld chunk contract.
- Check collision, interaction markers, spawns, transitions, and instance
  metadata.
- Do not assume a visually reachable tile is server-walkable.
- Verify transitions in both directions and with the correct instance policy.

Lua quests execute inside the server. Treat scripts as gameplay code: validate
all IDs, keep rewards server-derived, and test resumed quest state as well as a
fresh start.

## Client Platform Changes

Desktop and library entrypoints currently have separate frame runtimes. Until
they are unified, any change to boot, frame order, input, tutorial behavior,
rendering, networking, or lifecycle must be checked in:

- Desktop native
- Browser/WASM when affected
- Android when affected

Do not fix only one runtime and assume the other follows it.

Presentation prediction may improve responsiveness, but reconciliation must
converge on server state. Platform adapters may differ; gameplay behavior
should not.

## Frontend And Tooling Changes

The mapper API can write repository files. Validate paths using structured path
operations and enforce that resolved paths remain under an approved root. Do
not build filesystem paths by concatenating untrusted input.

React changes must pass TypeScript compilation and hooks lint. Avoid moving
editor behavior into the already large global Zustand store when a local store
or domain slice has clear ownership.

Large bundle warnings should be addressed with route or feature-level code
splitting when they affect initial load; do not hide the warning without
measuring.

## Required Validation

Run the checks relevant to your change. Before a merge to `master`, the target
is the full matrix:

```bash
(cd rust-server && cargo fmt --check && cargo test --all-targets)
(cd client && cargo fmt --check && cargo test --all-targets)
(cd launcher && cargo check --all-targets)
(cd mapper/server && npm run build)
(cd mapper && npm run build && npm run lint)
(cd web-stats && npm run build && npm run lint)
```

For server tick, synchronization, or large-system changes, also run:

```bash
cd rust-server
cargo test --release \
  game::load_test::full_tick_stays_within_budget_for_128_players \
  -- --ignored --nocapture
```

As of June 11, 2026, mapper lint has 28 pre-existing errors and 3 warnings.
Do not add new violations. Changes touching those files should reduce the
baseline, and lint must be cleaned before it becomes a hard CI gate.

Rust also has a substantial warning baseline. Do not introduce new warnings;
prefer removing warnings in files already being changed without mixing in a
repository-wide refactor.

## Tests

Match test scope to risk:

- Pure formulas and parsing: unit tests
- Registries and content loading: fixture-based loader tests
- Commands: authorization and invariant tests
- Protocol changes: encode/decode and golden frame tests
- Persistence: fresh schema, upgrade, save/load round trips, transaction failure
- Instances: cross-instance rejection and cleanup
- Networking: reconnect, takeover, duplicate connection, timeout, backpressure
- Client behavior: state reducer/handler tests separate from rendering
- Performance-sensitive systems: release-mode capacity tests

A test that only exercises the happy path is insufficient for an economy,
inventory, combat reward, trade, or persistence change.

## Pull Requests

Keep pull requests reviewable and explain:

- The behavior and ownership boundary being changed
- Compatibility impact on clients, protocol, database, and content
- Security assumptions and validation performed
- Tests and manual checks run
- Known limitations or follow-up work
- Screenshots or recordings for visible UI/map changes
- Performance measurements for tick-path or synchronization changes

Do not combine generated asset churn, formatting of unrelated files, a schema
change, and a gameplay feature unless they are inseparable.

## Release Checklist

Before releasing:

1. Confirm desktop and library clients use approved production HTTP and
   WebSocket endpoints.
2. Run the full validation matrix.
3. Run the 128-player capacity test for server simulation changes.
4. Test database upgrade on a copy of production-shaped data.
5. Validate required content and maps from a clean checkout.
6. Confirm mapper and operational endpoints are not publicly exposed.
7. Verify client package hashes and the launcher manifest.
8. Smoke-test login, character selection, matchmaking, reconnect, movement,
   combat, inventory, saving, and logout.
9. Verify rollback artifacts and database backup availability.

The current deployment workflow does not enforce these checks. Release owners
must perform them until CI and deployment gates are added.
