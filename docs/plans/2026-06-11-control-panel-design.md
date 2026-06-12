# Control Panel (Ops/Admin) Design

**Date:** 2026-06-11
**Status:** Approved, ready for implementation

## Goal

An authenticated ops/admin panel for inspecting live server health and world
state: API logs, performance metrics, room/instance population, a live player
list, and per-room entity snapshots — to help identify and debug issues in
production.

## Architecture

The panel is a **SvelteKit route** in the existing `/site` app. The rust-server
binary serves **no HTML** — it only gains a few admin-gated JSON endpoints. This
matches the existing convention where `/site`'s `/world/*` routes consume the
server's `/api/stats/*` endpoints.

Because the site fetches via a relative base (`/api/stats`), the site and API
are served same-origin (reverse proxy routes `/api/*` to the rust-server). The
panel therefore needs **no CORS work and no API base URL config** — it calls
relative `/api/...` paths with a bearer header.

### Site side (`/site`)

- New route `src/routes/control/+page.svelte` — the panel.
  - `src/routes/control/+page.ts` sets `export const ssr = false` and
    `export const prerender = false` so it is a pure client-side SPA route.
    adapter-static already has `fallback: '404.html'` for client routing.
- New `src/lib/control.ts` — typed API client mirroring `api.ts`, with an authed
  `get<T>(path, token)` that adds `Authorization: Bearer ${token}`.
- Token stored in `sessionStorage`, never prerendered.
- Tailwind v4 + lucide icons, matching the `/world/*` look.

### Server side (`rust-server`)

Add three new endpoints inside the existing
`if config.admin_api_token.is_some()` block in `main.rs`, each reusing the
existing `is_admin_request()` bearer-token check (constant-time compare against
`AEVEN_ADMIN_API_TOKEN`):

- `GET /api/admin/players`
- `GET /api/admin/rooms`
- `GET /api/admin/room/:room_id/entities`

`/api/logs` and `/api/perf` are reused untouched.

Handlers live alongside the existing ops handlers in `stats_api.rs`.

### Auth model

Reuse the existing bearer token (`AEVEN_ADMIN_API_TOKEN`). The `/control` page
is served (statically) without auth — it contains no secrets, just the login UI.
On load, JS reads any token from `sessionStorage`; if absent it shows a token
prompt. Submitting probes `/api/perf`: 200 → store token + render panel, 401 →
inline error. Every data fetch attaches `Authorization: Bearer <token>`. Any
later 401 clears the token and returns to the gate.

**Security note:** the `/control` URL is discoverable on the public site, but it
shows nothing without a valid token and all data endpoints enforce the bearer
server-side — same posture as serving from the binary.

## Endpoint data shapes

### `GET /api/admin/rooms`

Array of room summaries (iterates `state.rooms`):

```jsonc
[{
  "room_id": "overworld",
  "player_count": 12,
  "npc_count": 84,
  "overworld_players": 9,   // players in this room not in an instance
  "instance_players": 3
}]
```

### `GET /api/admin/players`

Flat list across all rooms (from `get_all_players()`). Shows **all** players
including admins (flagged); admins are NOT hidden (unlike `stats_online`).

```jsonc
[{
  "id": "p_abc", "name": "Thorin",
  "room_id": "overworld",
  "instance_id": "dungeon_42",   // null = overworld; from player_instances
  "x": 104, "y": -33, "z": 0,
  "hp": 31, "max_hp": 40,        // max_hp from skills.hitpoints.level
  "combat_level": 42,
  "active": true,                // websocket connected
  "is_dead": false,
  "target_id": "npc_9",          // who they're fighting, or null
  "is_admin": false, "is_god_mode": false,
  "ip_address": "1.2.3.4"        // included (useful for spotting multi-logging)
}]
```

### `GET /api/admin/room/:room_id/entities`

NPCs + players for one room:

```jsonc
{
  "room_id": "overworld",
  "npcs": [{
    "id": "npc_9", "prototype_id": "goblin",
    "display_name": "Goblin",
    "x": 100, "y": -30, "z": 0,
    "hp": 5, "max_hp": 12, "level": 7,
    "state": "Aggro",            // NpcState as string
    "target_id": "p_abc",
    "hidden": false, "invulnerable": false
  }],
  "players": [ /* same shape as /api/admin/players */ ]
}
```

## Panel UX & layout

Single route `/control`, gated by the token. Once authenticated, a tabbed
layout matching `/world/*` Tailwind styling.

**Login gate:** centered card with a password-type "Admin token" input; submit
probes `/api/perf`. 200 → store + enter; 401 → inline error. A "Lock" button in
the header clears the token.

**Tabs:**

| Tab | Source | Notes |
|-----|--------|-------|
| Overview | `/api/perf` | Load (rooms, players, overworld/instance split), tick spikes, derived rates. Headline health view. |
| Rooms | `/api/admin/rooms` | Sortable table; click a row → opens Entities tab scoped to that room. |
| Players | `/api/admin/players` | Sortable/filterable table: name, room/instance, pos, hp, combat, flags, ip. Text filter box. |
| Entities | `/api/admin/room/:id/entities` | Room picker + NPC table (proto, pos, hp, state, target) and the room's players. |
| Logs | `/api/logs` | Level filter (ERROR/WARN/INFO), "important only" toggle, count selector. Color-coded rows. |

**Refresh model:** shared "Auto-refresh" toggle (default on, ~3s interval) +
manual refresh button in the header. Each tab refetches its own data on the
tick; switching tabs fetches immediately. Polling only — no websocket/SSE.

**Error handling:** any 401 → clear token, return to gate. Network/500 →
non-blocking banner, keep showing last-good data so a blip doesn't wipe the view.

## Out of scope (YAGNI for now)

- Visual top-down/isometric world map render (dots for players/NPCs).
- Session-cookie or username/password auth (bearer token reuse is sufficient).
- WebSocket/SSE live streaming (polling is sufficient).
- Mutating actions (kick/ban/teleport from the panel) — read-only for now.

## Relevant existing code

- `rust-server/src/stats_api.rs` — `api_logs`, `api_perf`, `is_admin_request`,
  `constant_time_eq`, `stats_online` (player-listing pattern).
- `rust-server/src/main.rs` — router; `if config.admin_api_token.is_some()` block.
- `rust-server/src/config.rs` — `admin_api_token` from `AEVEN_ADMIN_API_TOKEN`.
- `rust-server/src/game.rs` — `Player` struct (line ~446).
- `rust-server/src/game/player_state.rs` — `get_all_players()` (~923),
  `get_all_npcs()` (~1229).
- `rust-server/src/npc.rs` — `Npc` struct (~100), `NpcState`.
- `rust-server/src/perf_metrics.rs` — `PerfSnapshot` shape.
- `site/src/lib/api.ts` — API client pattern to mirror.
- `site/src/routes/world/players/+page.svelte` — sortable table pattern.
