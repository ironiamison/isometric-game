# Repo Modularization Plan

## Goal

Reduce the maintenance cost of the largest client and server files without changing game behavior.

Success criteria:

- No single source file above roughly 1,500 to 2,000 lines unless it is mostly data/constants.
- No single function above roughly 200 to 300 lines unless it is a deliberate top-level orchestrator.
- Domain code grouped by behavior instead of by "everything in one file".
- Refactors done in small behavior-preserving steps with a build check after each step.

## Current hotspots

The biggest logic-heavy files today are:

| File | Lines | Functions | Main problem |
| --- | ---: | ---: | --- |
| `rust-server/src/game.rs` | 19,461 | 213 | Too many gameplay domains in one `GameRoom` implementation |
| `client/src/render/renderer.rs` | 9,972 | 91 | Rendering pipeline, asset loading, minimap, overlays, and UI helpers mixed together |
| `client/src/input/handler.rs` | 9,859 | 52 | One giant `InputHandler::process()` function handles nearly everything |
| `rust-server/src/protocol.rs` | 6,095 | 17 | Message encoding is concentrated in one giant serializer |
| `rust-server/src/main.rs` | 4,992 | 39 | HTTP routes, websocket flow, instance transitions, and bootstrapping mixed together |
| `client/src/network/message_handler.rs` | 4,104 | 8 | One giant `handle_room_data()` match handles all incoming server messages |
| `client/src/ui/screens.rs` | 3,256 | 37 | Login, character select, and character create screens all live together |

The heaviest individual functions are:

| File | Function | Lines | Notes |
| --- | --- | ---: | --- |
| `client/src/input/handler.rs` | `process` | 8,317 | Highest priority client refactor target by a large margin |
| `client/src/network/message_handler.rs` | `handle_room_data` | 3,973 | Natural split by message family and by `stateSync` sub-handler |
| `rust-server/src/game.rs` | `tick` | 2,861 | Should become tick phases or submodules |
| `rust-server/src/protocol.rs` | `encode_server_message` | 3,488 | Should be split by message type/family |
| `client/src/render/renderer.rs` | `new` | 1,244 | Asset loading/bootstrap logic is too broad |
| `rust-server/src/game.rs` | `handle_attack` | 955 | Combat rules and effects are overly concentrated |
| `rust-server/src/game.rs` | `handle_chat_command` | 851 | Command dispatch and command implementations are mixed |
| `rust-server/src/game.rs` | `cast_damage_spell_resolved` | 849 | Spell resolution needs its own domain file |

## What to split first

Prioritize files where a small structural change will remove the most complexity:

1. `client/src/input/handler.rs`
2. `client/src/network/message_handler.rs`
3. `rust-server/src/game.rs`
4. `client/src/ui/screens.rs`
5. `rust-server/src/main.rs`
6. `client/src/render/renderer.rs`
7. `rust-server/src/protocol.rs`

That order is deliberate:

- `input/handler.rs` and `message_handler.rs` have the clearest extraction seams and the highest immediate readability payoff.
- `game.rs` is the biggest long-term win, but it touches many systems and should start after the client monoliths are under control.
- `renderer.rs` is large, but the repo already split some UI rendering into `client/src/render/ui/*.rs`, so there is already a pattern to extend.
- `protocol.rs` is important, but wire-format code benefits from being stabilized after domain splits settle.

## Extraction strategy by file

### 1. `client/src/input/handler.rs`

The main issue is not file size alone. It is the 8,317-line `process()` function.

Refactor in two passes:

Pass A: split the giant function into private methods in the same file first.

Candidate methods:

- `update_touch_controls(...)`
- `update_hover_state(...)`
- `handle_drag_drop(...)`
- `handle_chat_input(...)`
- `handle_dialogue_input(...)`
- `handle_panel_clicks(...)`
- `handle_world_clicks(...)`
- `handle_minimap_input(...)`
- `handle_hotkeys(...)`
- `handle_movement_input(...)`
- `handle_auto_actions(...)`

Pass B: move those methods into submodules.

Suggested layout:

```text
client/src/input/handler/
  mod.rs
  chat.rs
  drag_drop.rs
  interaction.rs
  minimap.rs
  movement.rs
  panels.rs
  touch_input.rs
```

### 2. `client/src/network/message_handler.rs`

This is a giant message dispatcher. It should become a small dispatcher plus domain handlers.

Split by message family:

- `welcome.rs`
- `players.rs`
- `state_sync.rs`
- `combat.rs`
- `inventory.rs`
- `quests.rs`
- `dialogue.rs`
- `shops.rs`
- `social.rs`
- `transitions.rs`

Suggested layout:

```text
client/src/network/message_handler/
  mod.rs
  combat.rs
  dialogue.rs
  inventory.rs
  players.rs
  quests.rs
  shops.rs
  social.rs
  state_sync.rs
  transitions.rs
  welcome.rs
```

Important detail:

- `stateSync` should be its own handler first. It contains a large internal update pipeline that can be further split into player sync, NPC sync, world sync, and transient event sync.

### 3. `rust-server/src/game.rs`

This file is carrying multiple domains inside a single `GameRoom` impl.

Do not start by moving everything at once. First convert `game.rs` into a module directory and keep `GameRoom` as the public anchor.

Suggested layout:

```text
rust-server/src/game/
  mod.rs
  room.rs
  tick.rs
  combat.rs
  spells.rs
  chat.rs
  commands.rs
  npc_interactions.rs
  crafting.rs
  gathering.rs
  shops.rs
  equipment.rs
  sync.rs
```

Suggested mapping:

- `tick.rs`: `tick`, respawn flow, regen, NPC updates, sync broadcast orchestration
- `combat.rs`: `handle_attack`, target validation, hit/damage resolution
- `spells.rs`: `cast_damage_spell_resolved` and spell effect helpers
- `chat.rs`: plain chat handling and nearby/global broadcast logic
- `commands.rs`: `handle_chat_command` plus admin command implementations
- `npc_interactions.rs`: `handle_npc_interact`, dialogue, quest offers, offerings
- `crafting.rs`: `check_burn`, `handle_craft`, `handle_start_craft`, batch crafting
- `gathering.rs`: woodcutting, mining, harvesting, gathering state transitions
- `shops.rs`: buy/sell/open shop flows
- `equipment.rs`: equip/unequip/stat recalculation
- `sync.rs`: state sync helpers and outbound room update helpers

Rust allows multiple `impl GameRoom` blocks across module files, which is the cleanest migration path here.

### 4. `client/src/ui/screens.rs`

This file already has a natural split by screen type.

Suggested layout:

```text
client/src/ui/screens/
  mod.rs
  common.rs
  login.rs
  character_select.rs
  character_create.rs
```

Move shared helpers into `common.rs`:

- sprite/font loading helpers
- preview drawing helpers
- shared text input helpers where practical

### 5. `rust-server/src/main.rs`

This file mixes startup, REST routes, websocket session lifecycle, spectator mode, and instance transitions.

Suggested layout:

```text
rust-server/src/server/
  mod.rs
  app_state.rs
  auth.rs
  characters.rs
  matchmaking.rs
  stats.rs
  websocket.rs
  spectator.rs
  instances.rs
  bootstrap.rs
```

Suggested mapping:

- `app_state.rs`: `AppState`, `GameSession`, `RateLimiter`, token signer
- `auth.rs`: register/login/logout and header extraction
- `characters.rs`: list/create/delete character routes
- `matchmaking.rs`: join/create room route
- `websocket.rs`: `ws_handler`, `handle_socket`, client message dispatch
- `spectator.rs`: spectator websocket path and stream loop
- `instances.rs`: portal enter/auto-enter helpers
- `stats.rs`: stats/log/perf endpoints
- `bootstrap.rs`: router creation and startup wiring

### 6. `client/src/render/renderer.rs`

This file is large in a more manageable way than `input/handler.rs`. The repo already split some UI panels into `client/src/render/ui/*.rs`, which is the right pattern.

Next splits should focus on non-UI domains:

```text
client/src/render/
  renderer/
    mod.rs
    assets.rs
    world.rs
    entities.rs
    minimap.rs
    overlays.rs
    text.rs
```

Suggested mapping:

- `assets.rs`: asset loading, sprite stores, atlas helpers, `Renderer::new`
- `world.rs`: tilemap layers, depth-sorted collection, world object rendering
- `entities.rs`: player/NPC/item rendering
- `minimap.rs`: minimap bounds, markers, preview, overlay rendering
- `overlays.rs`: announcements, death overlay, chat bubbles, floating events
- `text.rs`: text measurement/caching helpers

### 7. `rust-server/src/protocol.rs`

This one is less about file movement and more about serializer organization.

Suggested split:

```text
rust-server/src/protocol/
  mod.rs
  decode.rs
  encode/
    mod.rs
    combat.rs
    inventory.rs
    player.rs
    quest.rs
    social.rs
    world.rs
```

Keep the public API stable:

- `encode_server_message(...)`
- `decode_client_message(...)`

Internally dispatch to family-specific encoders.

## Refactor workflow

For each target file, use the same sequence:

1. Freeze behavior.
2. Add or identify one narrow seam.
3. Extract private helper methods without moving files yet.
4. Compile.
5. Move helpers into submodules with no behavior changes.
6. Compile again.
7. Only after the structure is clean, consider domain cleanup or API cleanup.

Rules:

- Never combine feature work with structural refactors.
- One domain per PR or commit.
- Prefer moving code unchanged before rewriting it.
- Keep public entry points stable while internals move.

## Audit process to repeat during the refactor

Before each round, regenerate a simple hotspot report:

1. Rank files by line count.
2. Rank functions by rough line span.
3. Flag single functions above 200 lines.
4. Flag files where one function owns more than 40 percent of the file.
5. Flag modules with more than one gameplay domain mixed together.

Useful heuristics:

- File size threshold: 1,500 lines
- Function size threshold: 200 lines
- "Critical" function threshold: 500 lines
- One-file dispatcher threshold: one `match` handling more than 15 to 20 message types

## Recommended execution plan

### Phase 0: guardrails

- Add a repeatable audit script or `just` target for file/function hotspot reporting.
- Decide on target thresholds for file and function size.
- Pick one compilation command per crate and run it after every extraction step.

### Phase 1: client interaction cleanup

- Split `InputHandler::process()` into private methods.
- Move those methods into `client/src/input/handler/`.
- Split `handle_room_data()` into family handlers.
- Extract `stateSync` handling first because it is the largest sub-domain.

### Phase 2: screen and route decomposition

- Split `client/src/ui/screens.rs` into per-screen modules.
- Split `rust-server/src/main.rs` into route/websocket/bootstrap modules.

### Phase 3: server gameplay decomposition

- Convert `rust-server/src/game.rs` into `rust-server/src/game/`.
- Move chat/commands out first.
- Move gathering/crafting/shop/equipment handlers out next.
- Split `tick()` last into phase functions once surrounding helpers already live in modules.

### Phase 4: renderer and protocol cleanup

- Extract renderer asset loading and minimap logic.
- Extract overlay rendering.
- Split protocol encode/decode by message family while keeping public API unchanged.

### Phase 5: finish and enforce

- Re-run the hotspot audit.
- Add CI or a lightweight local check for oversized files/functions.
- Document the expected module boundaries in `ARCHITECTURE.md`.

## Risks to watch

- Large `GameRoom` refactors can create borrow-checker churn. Move small groups of methods at a time.
- Message handler splits can accidentally change ordering-sensitive state mutations. Keep handlers thin and preserve current call order.
- Renderer splits can break hidden shared caches or implicit font/material state. Extract read-only helpers first.
- Input refactors can easily change interaction priority. Preserve current ordering exactly during the first pass.

## Definition of done

The refactor is successful when:

- The current top 7 hotspot files are reduced or converted into directories.
- The current giant single-function choke points are gone.
- The public client/server behavior is unchanged.
- New gameplay features can be added by touching one domain module instead of one monolith.
