# Contributing To Aeven

Aeven is a live multiplayer system. A local-looking change can affect authority,
wire compatibility, persistent characters, map references, three client
platforms, privileged tooling, or production delivery.

Read [ARCHITECTURE.md](ARCHITECTURE.md) before structural work.

## Toolchain

Required versions are repository policy:

- Rust `1.92.0`
- Node.js `22.x`
- npm `10.x`
- Python `3.11+` for packaging/assets

Use the checked-in toolchain files and lockfiles:

```bash
rustup show
node --version
npm --version

(cd mapper && npm ci)
(cd mapper/server && npm ci)
(cd site && npm ci)
```

Do not replace `npm ci` with an unreviewed lockfile refresh. Do not run Cargo
without `--locked` in CI, release, or verification commands.

## Working Agreement

Before editing, identify every affected boundary:

- Client command or server event
- Protocol compatibility/version
- Persisted schema or serialized state
- Server authority and instance scope
- Tick phase, lock scope, channel capacity, or sync bandwidth
- Content IDs referenced by maps, loot, quests, shops, or saves
- Desktop, WASM, or Android lifecycle
- Public HTTP, authenticated ops, or mapper mutation surface
- Packaging, endpoint configuration, or release automation

Keep gameplay changes separate from unrelated cleanup. Preserve released IDs
and semantics unless the change includes an explicit migration/version plan.

## Server Authority

Clients send intent. The server derives results.

Every mutating command must validate the relevant combination of:

- Authenticated account and active character
- Current command lease/connection generation
- Character, entity, item, object, or activity ownership
- Room and instance membership
- Position, proximity, collision, and line of sight
- Cooldown, action state, sequence, and stale input
- Quantity, price, inventory capacity, and resources
- Skill, quest, Slayer, equipment, spell, and prayer requirements
- Duplicate, replayed, stale, and out-of-order requests

Never accept client-calculated damage, XP, rewards, prices, loot, drops,
completion, target access, or final position.

Choose publication scope explicitly:

- **Unicast:** inventory, bank, equipment, progression, private errors
- **Spatial/instance:** movement, combat, nearby entities and items
- **Room:** events every player in that room must receive
- **Global:** rare process-wide announcements only

## Domain Ownership

`GameRoom` is an aggregate root, not a default home for new state.

A new server system should normally provide:

- A type that owns mutable domain state
- Validated command methods
- Narrow read-only queries or snapshots
- Domain events/protocol projections
- A defined tick phase and measured cost, if applicable
- A persistence snapshot/change set, if durable
- Content/reference validation, if data-driven

Do not expose `RwLock`/`Mutex` guards as module APIs. Do not add a long positional
constructor parameter list; group cohesive dependencies in a service or
validated registry object.

On the client, keep replicated truth, presentation effects, input, and UI state
separate. All platforms must construct gameplay through
`app::new_game_state`/`configure_game_state` and run active gameplay through
`run_game_frame`.

## Async And Concurrency

- Do not hold a lock across `.await` unless a documented invariant requires it.
- Snapshot small values before database, filesystem, compression, or network I/O.
- Define lock order when one operation needs multiple locks.
- Keep tick phases free of blocking filesystem/database work.
- Use bounded channels for connection work.
- Treat dropped bootstrap/private state as correctness failures.
- Record and handle backpressure instead of silently growing queues.
- Serialize same-character admission/takeover and same-resource mutations.

Any change to tick, synchronization, connection channels, AI, visibility, or
large shared collections must run the release capacity gate.

## Error Policy

Classify errors deliberately:

- **Fatal startup:** invalid production config, failed migration, malformed
  required content/map, broken content graph
- **Command rejection:** invalid or unauthorized player intent
- **Connection failure:** malformed protocol, timeout, invalid lease,
  backpressure/send failure
- **Best effort:** optional telemetry or cleanup that cannot affect game truth

Do not discard `Result` using `.ok()` or `let _ =` unless the operation is
genuinely best effort and the reason is evident. Logs must not contain
passwords, bearer tokens, signed room tokens, private chat, or unnecessary
personal information.

## Protocol Changes

### Client To Server

Commands are defined once in `crates/aeven-protocol`.

1. Add or change the `ClientMessage` variant with an explicit stable name.
2. Add codec round-trip and malformed-input tests.
3. Add aliases only when preserving a released field spelling.
4. Add server dispatch and domain validation.
5. Test unauthorized, stale, malformed, and cross-instance cases.
6. Update every client call site.

Do not duplicate a command DTO in client/server modules.

### Server To Client

Server events are optimized projections:

1. Add/update the server message variant and encoder.
2. Select unicast/spatial/room/global scope.
3. Update the client handler and smallest owning state module.
4. Test encoded field names/types and representative client decoding.
5. Keep old fields readable when compatibility is practical.

### Versioning

Increment `aeven_protocol::PROTOCOL_VERSION` when an old client or server cannot
correctly interoperate. Examples:

- Removing/renaming a message or required field
- Changing a field type, unit, meaning, or default
- Changing ordering/atomicity semantics that affect gameplay correctness

Additive optional fields with safe defaults may remain on the same version.
Never reuse an old message name for different semantics.

## Database And Persistence

Migrations live in `rust-server/migrations/`.

- Never edit a migration already used by a released server.
- Add the next zero-padded numbered migration.
- Make the migration deterministic and transactional where SQLite permits.
- Test a fresh database and an upgrade from the previous released schema.
- Keep related row changes in one transaction.
- Preserve old JSON fields with Serde defaults or migrate them explicitly.
- Keep compatibility upgrades narrowly scoped and introspected.
- Never manually patch the production database as the normal deployment path.

Autosave snapshots must be collected without holding room locks during SQL.
Persistence failures affecting character truth must be surfaced and measured.

## Content Changes

Authoritative data lives in `rust-server/data/` and `rust-server/maps/`.

Rules:

- Released item, entity, quest, recipe, chest, interior, and object IDs are stable.
- Every referenced ID must exist.
- Duplicate IDs and duplicate unique spawn IDs are errors.
- Quantities, probabilities, weights, dimensions, and coordinates are bounded.
- Empty required registries are errors.
- Required files never fall back to empty/default production behavior.
- Lua rewards and transitions remain server-derived.

Run:

```bash
cargo test --locked -p isometric-server production_content_registries_load
cargo test --locked -p isometric-server production_room_bootstrap_loads_all_runtime_content
```

The first validates immutable registries and cross-references. The second
constructs the complete production room, loads every chunk/runtime subsystem,
and catches integration-only content failures.

### Maps

Overworld chunks must:

- Use format version `2`
- Be exactly `32x32`
- Match filename and payload coordinates
- Contain exactly 1,024 entries in each tile layer
- Contain exactly 128 decoded collision bytes
- Use registered entities and gathering zones
- Reference existing interior maps and named spawns

Interiors must have valid dimensions, layers, collision, spawn points, portals,
entities, chests, and optional elevation arrays.

Use the mapper for structured edits. Review generated JSON/TOML and atlas diffs
for unrelated churn before committing.

## Mapper And Web Security

The mapper API can alter repository and deployable files.

- Keep the default bind address loopback-only.
- Production requires `MAPPER_AUTH_SECRET` of at least 32 characters.
- Production users require scrypt `passwordHash`; plaintext is development-only
  and requires `MAPPER_ALLOW_PLAINTEXT_PASSWORDS=1`.
- Validate world access on every world-scoped route.
- Require CSRF for every state-changing API.
- Resolve paths beneath approved roots; never concatenate untrusted paths.
- Validate payload shape/size before the first filesystem mutation.
- Use atomic replacement for JSON and directory-level bulk writes.
- Serialize asset rebuilds and related multi-file mutations.
- Never invoke shell parsing with user-controlled arguments.

Frontend changes must pass TypeScript/Svelte checks and lint. Treat accessibility
warnings as failures. Split large entry bundles when measurement shows startup
cost, rather than suppressing warnings.

## Configuration And Secrets

Do not commit:

- Passwords or password hashes for real users
- Bearer/admin/session secrets
- `.env` files
- Production databases
- SSH/R2 credentials
- Generated local mapper users

Production server requirements:

- `AEVEN_ENV=production`
- `AEVEN_SESSION_SIGNING_SECRET` at least 32 bytes
- Exact HTTPS CORS origins
- `AEVEN_ADMIN_API_TOKEN` only when ops routes are needed
- `AEVEN_TRUSTED_PROXIES` only for proxies actually in the request path

Release clients require HTTPS/WSS and reject localhost unless an intentional
private build explicitly opts out.

## Tests

Match test scope to blast radius:

- **Unit:** parsing, validation, calculations, state transitions
- **Protocol:** round trips, aliases, malformed/oversized frames, version mismatch
- **Integration:** authority, ownership, instance isolation, persistence
- **Production bootstrap:** all registries/maps/runtime content
- **Capacity:** tick/sync/AI/shared-state changes
- **Frontend/API:** auth, CSRF, path scope, validation, atomic writes

Bug fixes should include a regression test when the behavior is testable.

## Required Verification

Run the complete matrix before merging:

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

For server tick, sync, transport, instance, AI, or high-fanout changes:

```bash
cd rust-server
cargo test --locked --release -p isometric-server \
  full_tick_stays_within_budget_for_128_players \
  -- --ignored --nocapture
```

Do not weaken lint levels, ignore a failing audit, raise the tick budget, or
disable a validation test to merge a change.

## Pull Requests

A pull request should state:

- User-visible and architectural behavior changed
- Authority/security implications
- Protocol version/compatibility implications
- Migration/content ID implications
- Platforms exercised
- Exact verification commands and results
- Capacity result when required
- Deployment or rollback considerations

Automatic deployment and release workflows run only after successful CI on
`master`. A green deploy job is not a substitute for the local review above.
