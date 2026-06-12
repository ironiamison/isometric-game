# Control Panel (Ops/Admin) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an authenticated ops/admin panel — a SvelteKit `/control` route in `/site` backed by three new bearer-gated JSON endpoints on the rust-server — for inspecting live logs, perf, room population, players, and per-room entities.

**Architecture:** The rust-server serves no HTML; it only gains JSON endpoints in a new `admin_api.rs` module, each reusing the existing `is_admin_request()` bearer check (`AEVEN_ADMIN_API_TOKEN`). The panel is a pure client-side SvelteKit route that fetches relative `/api/...` paths (same-origin in prod, so no CORS work) with an `Authorization: Bearer` header, with the token held in `sessionStorage`.

**Tech Stack:** Rust (Axum, Serde), SvelteKit 5 (runes), TypeScript, Tailwind v4, lucide-svelte.

**Design doc:** `docs/plans/2026-06-11-control-panel-design.md`

**Working directory:** `/Users/samson/projects/isometric-game/.worktrees/control-panel` (branch `feature/control-panel`).

---

## Testing strategy (read first)

- **Rust:** The existing ops handlers (`api_perf`, `api_logs`) have **no unit tests** because they need a full `AppState` (DB + registries) that is expensive to construct. We follow the same reality:
  - The one genuinely bug-prone pure function — `npc_state_label(NpcState) -> &str` (exhaustive 9-variant match) — gets a real TDD unit test.
  - The handlers themselves (thin field-copy mappers over live state) are verified by `cargo build` + a manual `curl` smoke test with the bearer token, documented per task. Do **not** build giant `Player` fixtures (no `Default`, ~50 fields) just to test field copies — that is fixture theater, not testing.
- **Site:** `/site` has no test framework (only `svelte-check`). Adding one is out of scope (YAGNI). Frontend verification = `npm run check` (type check) + a manual browser smoke test, documented per task.

Set the token for manual testing once per shell:
```bash
export ADMIN_TOKEN="<value of AEVEN_ADMIN_API_TOKEN you run the server with>"
```
Run the server (in the worktree) for smoke tests:
```bash
cd rust-server && AEVEN_ADMIN_API_TOKEN="$ADMIN_TOKEN" cargo run
# leave running in a second terminal; endpoints are at http://localhost:<port>
```
Confirm the listen port from server startup logs before curling.

---

## Task 1: New `admin_api` module with view types + `npc_state_label` (TDD)

**Files:**
- Create: `rust-server/src/admin_api.rs`
- Modify: `rust-server/src/main.rs` (add `mod admin_api;`)
- Modify: `rust-server/src/stats_api.rs` (make `is_admin_request` shareable)

**Step 1: Make `is_admin_request` shareable.**
In `rust-server/src/stats_api.rs`, change the signature:
```rust
// was: fn is_admin_request(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
pub(super) fn is_admin_request(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
```
(Leave `constant_time_eq` as-is.)

**Step 2: Create the module with view types and the label fn + its failing test.**
Create `rust-server/src/admin_api.rs`:
```rust
use super::*;
use crate::npc::{Npc, NpcState};

// ============================================================================
// Admin/Ops API view types (read-only snapshots of live game state)
// ============================================================================

#[derive(Serialize)]
pub(super) struct AdminRoomSummary {
    pub room_id: String,
    pub player_count: usize,
    pub npc_count: usize,
    pub overworld_players: usize,
    pub instance_players: usize,
}

#[derive(Serialize)]
pub(super) struct AdminPlayer {
    pub id: String,
    pub name: String,
    pub room_id: String,
    pub instance_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub combat_level: i32,
    pub active: bool,
    pub is_dead: bool,
    pub target_id: Option<String>,
    pub is_admin: bool,
    pub is_god_mode: bool,
    pub ip_address: Option<String>,
}

#[derive(Serialize)]
pub(super) struct AdminNpc {
    pub id: String,
    pub prototype_id: String,
    pub display_name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub state: String,
    pub target_id: Option<String>,
    pub hidden: bool,
    pub invulnerable: bool,
}

#[derive(Serialize)]
pub(super) struct AdminRoomEntities {
    pub room_id: String,
    pub npcs: Vec<AdminNpc>,
    pub players: Vec<AdminPlayer>,
}

/// Stable string label for an NPC AI state (do not rely on Debug formatting,
/// which could change silently).
pub(super) fn npc_state_label(state: NpcState) -> &'static str {
    match state {
        NpcState::Idle => "Idle",
        NpcState::Chasing => "Chasing",
        NpcState::Attacking => "Attacking",
        NpcState::Returning => "Returning",
        NpcState::Dead => "Dead",
        NpcState::Wandering => "Wandering",
        NpcState::Submerging => "Submerging",
        NpcState::Emerging => "Emerging",
        NpcState::Burrowing => "Burrowing",
    }
}

/// Build an admin NPC view from a live NPC.
pub(super) fn admin_npc_from(npc: &Npc) -> AdminNpc {
    AdminNpc {
        id: npc.id.clone(),
        prototype_id: npc.prototype_id.clone(),
        display_name: npc.stats.display_name.clone(),
        x: npc.x,
        y: npc.y,
        z: npc.z,
        hp: npc.hp,
        max_hp: npc.max_hp,
        level: npc.level,
        state: npc_state_label(npc.state).to_string(),
        target_id: npc.target_id.clone(),
        hidden: npc.hidden,
        invulnerable: npc.invulnerable,
    }
}

/// Build an admin player view from a live player and the instance map snapshot.
pub(super) fn admin_player_from(
    player: &Player,
    room_id: &str,
    instance_id: Option<String>,
) -> AdminPlayer {
    AdminPlayer {
        id: player.id.clone(),
        name: player.name.clone(),
        room_id: room_id.to_string(),
        instance_id,
        x: player.x,
        y: player.y,
        z: player.z,
        hp: player.hp,
        max_hp: player.max_hp(),
        combat_level: player.skills.combat_level(),
        active: player.active,
        is_dead: player.is_dead,
        target_id: player.target_id.clone(),
        is_admin: player.is_admin,
        is_god_mode: player.is_god_mode,
        ip_address: player.ip_address.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_state_label_covers_all_variants() {
        assert_eq!(npc_state_label(NpcState::Idle), "Idle");
        assert_eq!(npc_state_label(NpcState::Chasing), "Chasing");
        assert_eq!(npc_state_label(NpcState::Attacking), "Attacking");
        assert_eq!(npc_state_label(NpcState::Returning), "Returning");
        assert_eq!(npc_state_label(NpcState::Dead), "Dead");
        assert_eq!(npc_state_label(NpcState::Wandering), "Wandering");
        assert_eq!(npc_state_label(NpcState::Submerging), "Submerging");
        assert_eq!(npc_state_label(NpcState::Emerging), "Emerging");
        assert_eq!(npc_state_label(NpcState::Burrowing), "Burrowing");
    }
}
```

**Step 3: Register the module.** In `rust-server/src/main.rs`, add alongside the other `mod` lines (keep alphabetical-ish; place after `mod admin`-less neighbors, e.g. right after `mod app_state;`):
```rust
mod admin_api;
```

**Step 4: Run the test — expect PASS (it compiles the exhaustive match).**
```bash
cd rust-server && cargo test -p isometric-server admin_api::tests::npc_state_label_covers_all_variants
```
Expected: `test result: ok. 1 passed`. If `NpcState` gained a variant, the match fails to compile — that is the intended tripwire.

**Step 5: Commit.**
```bash
git add rust-server/src/admin_api.rs rust-server/src/main.rs rust-server/src/stats_api.rs
git commit -m "feat(server): admin_api view types + npc_state_label"
```

---

## Task 2: `GET /api/admin/rooms` handler

**Files:**
- Modify: `rust-server/src/admin_api.rs` (add handler)
- Modify: `rust-server/src/main.rs` (add route)

**Step 1: Add the handler** to `admin_api.rs`:
```rust
pub(super) async fn api_admin_rooms(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if !stats_api::is_admin_request(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let instances = state.player_instances.read().await.clone();
    let mut out = Vec::new();
    for entry in state.rooms.iter() {
        let room = entry.value();
        let players = room.get_all_players().await;
        let npc_count = room.get_all_npcs().await.len();
        let instance_players = players
            .iter()
            .filter(|p| instances.contains_key(&p.id))
            .count();
        out.push(AdminRoomSummary {
            room_id: room.id.clone(),
            player_count: players.len(),
            npc_count,
            overworld_players: players.len() - instance_players,
            instance_players,
        });
    }
    Json(out).into_response()
}
```
> Note: `get_all_npcs().await.len()` clones NPCs just to count. Acceptable for an admin-frequency endpoint; do not add a new accessor for this (YAGNI).

**Step 2: Wire the route** in `main.rs`, inside the existing `if config.admin_api_token.is_some()` block (next to `/api/perf` and `/api/logs`):
```rust
.route("/api/admin/rooms", get(admin_api::api_admin_rooms))
```

**Step 3: Build.**
```bash
cd rust-server && cargo build -p isometric-server
```
Expected: compiles.

**Step 4: Manual smoke test** (server running per the testing-strategy section):
```bash
# Unauthorized → 401
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:<port>/api/admin/rooms
# Authorized → 200 + JSON array
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:<port>/api/admin/rooms | head -c 400
```
Expected: first prints `401`; second prints a JSON array (e.g. `[{"room_id":"overworld",...}]`).

**Step 5: Commit.**
```bash
git add rust-server/src/admin_api.rs rust-server/src/main.rs
git commit -m "feat(server): GET /api/admin/rooms"
```

---

## Task 3: `GET /api/admin/players` handler

**Files:**
- Modify: `rust-server/src/admin_api.rs`
- Modify: `rust-server/src/main.rs`

**Step 1: Add the handler:**
```rust
pub(super) async fn api_admin_players(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if !stats_api::is_admin_request(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let instances = state.player_instances.read().await.clone();
    let mut out = Vec::new();
    for entry in state.rooms.iter() {
        let room = entry.value();
        let room_id = room.id.clone();
        for p in room.get_all_players().await {
            let instance_id = instances.get(&p.id).cloned();
            out.push(admin_player_from(&p, &room_id, instance_id));
        }
    }
    Json(out).into_response()
}
```
> Note: `get_all_players()` returns only `active` (connected) players — exactly what we want for a live panel. Admins are **included** (flagged via `is_admin`), unlike `stats_online`.

**Step 2: Wire the route** in `main.rs` (same block):
```rust
.route("/api/admin/players", get(admin_api::api_admin_players))
```

**Step 3: Build.** `cd rust-server && cargo build -p isometric-server` — expect compiles.

**Step 4: Manual smoke test:**
```bash
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:<port>/api/admin/players | head -c 600
```
Expected: JSON array of players with `id,name,room_id,instance_id,x,y,z,hp,max_hp,combat_level,...,ip_address`. (Log in with a game client first so the list is non-empty.)

**Step 5: Commit.**
```bash
git add rust-server/src/admin_api.rs rust-server/src/main.rs
git commit -m "feat(server): GET /api/admin/players"
```

---

## Task 4: `GET /api/admin/room/:room_id/entities` handler

**Files:**
- Modify: `rust-server/src/admin_api.rs`
- Modify: `rust-server/src/main.rs`

**Step 1: Add the handler:**
```rust
pub(super) async fn api_admin_room_entities(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(room_id): Path<String>,
) -> axum::response::Response {
    if !stats_api::is_admin_request(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(room) = state.rooms.get(&room_id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let instances = state.player_instances.read().await.clone();
    let npcs = room.get_all_npcs().await.iter().map(admin_npc_from).collect();
    let players = room
        .get_all_players()
        .await
        .iter()
        .map(|p| admin_player_from(p, &room_id, instances.get(&p.id).cloned()))
        .collect();
    Json(AdminRoomEntities { room_id, npcs, players }).into_response()
}
```

**Step 2: Wire the route** in `main.rs` (same block):
```rust
.route(
    "/api/admin/room/:room_id/entities",
    get(admin_api::api_admin_room_entities),
)
```

**Step 3: Build.** `cd rust-server && cargo build -p isometric-server` — expect compiles.

**Step 4: Manual smoke test** (use a real room_id from the `/api/admin/rooms` output, e.g. `overworld`):
```bash
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:<port>/api/admin/room/overworld/entities | head -c 600
# unknown room → 404
curl -s -o /dev/null -w "%{http_code}\n" -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:<port>/api/admin/room/nope/entities
```
Expected: first prints `{"room_id":"overworld","npcs":[...],"players":[...]}`; second prints `404`.

**Step 5: Commit.**
```bash
git add rust-server/src/admin_api.rs rust-server/src/main.rs
git commit -m "feat(server): GET /api/admin/room/:room_id/entities"
```

---

## Task 5: Site — authed API client (`control.ts`)

**Files:**
- Create: `site/src/lib/control.ts`

**Step 1: Create the client.** Mirrors `src/lib/api.ts` but injects the bearer header and throws a typed `UnauthorizedError` on 401 so the UI can bounce to the gate.
```ts
// Authed client for the /control ops panel. All paths are same-origin relative.

export class UnauthorizedError extends Error {
  constructor() {
    super('Unauthorized');
    this.name = 'UnauthorizedError';
  }
}

export interface PerfSnapshot {
  // Loosely typed: we render a subset and JSON-dump the rest.
  current_load: {
    rooms: number;
    connected_players: number;
    overworld_players: number;
    instance_players: number;
    spectators?: number;
  };
  recent_spikes: { context: string; [k: string]: unknown }[];
  [k: string]: unknown;
}

export interface LogEntry {
  level: string;
  message: string;
  timestamp?: string;
  [k: string]: unknown;
}

export interface AdminRoomSummary {
  room_id: string;
  player_count: number;
  npc_count: number;
  overworld_players: number;
  instance_players: number;
}

export interface AdminPlayer {
  id: string;
  name: string;
  room_id: string;
  instance_id: string | null;
  x: number;
  y: number;
  z: number;
  hp: number;
  max_hp: number;
  combat_level: number;
  active: boolean;
  is_dead: boolean;
  target_id: string | null;
  is_admin: boolean;
  is_god_mode: boolean;
  ip_address: string | null;
}

export interface AdminNpc {
  id: string;
  prototype_id: string;
  display_name: string;
  x: number;
  y: number;
  z: number;
  hp: number;
  max_hp: number;
  level: number;
  state: string;
  target_id: string | null;
  hidden: boolean;
  invulnerable: boolean;
}

export interface AdminRoomEntities {
  room_id: string;
  npcs: AdminNpc[];
  players: AdminPlayer[];
}

async function get<T>(path: string, token: string): Promise<T> {
  const r = await fetch(path, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (r.status === 401) throw new UnauthorizedError();
  if (!r.ok) throw new Error(`API error: ${r.status}`);
  return r.json();
}

export const control = {
  // Used by the login gate to validate a token (200 = valid).
  perf: (token: string) => get<PerfSnapshot>('/api/perf', token),
  logs: (token: string, opts: { count?: number; level?: string; important?: boolean } = {}) => {
    const q = new URLSearchParams();
    if (opts.count != null) q.set('count', String(opts.count));
    if (opts.level) q.set('level', opts.level);
    if (opts.important) q.set('important', 'true');
    const qs = q.toString();
    return get<LogEntry[]>(`/api/logs${qs ? `?${qs}` : ''}`, token);
  },
  rooms: (token: string) => get<AdminRoomSummary[]>('/api/admin/rooms', token),
  players: (token: string) => get<AdminPlayer[]>('/api/admin/players', token),
  roomEntities: (token: string, roomId: string) =>
    get<AdminRoomEntities>(`/api/admin/room/${encodeURIComponent(roomId)}/entities`, token),
};

const TOKEN_KEY = 'aeven_control_token';
export const tokenStore = {
  get: () => (typeof sessionStorage === 'undefined' ? null : sessionStorage.getItem(TOKEN_KEY)),
  set: (t: string) => sessionStorage.setItem(TOKEN_KEY, t),
  clear: () => sessionStorage.removeItem(TOKEN_KEY),
};
```

**Step 2: Type-check.**
```bash
cd site && npm run check
```
Expected: 0 errors (warnings about unused exports are fine until the page uses them; if `npm run check` flags unused, ignore — they are consumed in later tasks).

**Step 3: Commit.**
```bash
git add site/src/lib/control.ts
git commit -m "feat(site): authed control-panel API client"
```

---

## Task 6: Site — `/control` route scaffold + auth gate

**Files:**
- Create: `site/src/routes/control/+page.ts`
- Create: `site/src/routes/control/+page.svelte`

**Step 1: Disable SSR/prerender for the route.** Create `site/src/routes/control/+page.ts`:
```ts
export const ssr = false;
export const prerender = false;
```

**Step 2: Create the gate + shell.** Create `site/src/routes/control/+page.svelte`. This task implements ONLY the auth gate and an empty authenticated shell with a Lock button; tabs come next.
```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { control, tokenStore, UnauthorizedError } from '$lib/control';
  import { Lock, LogIn } from 'lucide-svelte';

  let token = $state<string | null>(null);
  let tokenInput = $state('');
  let error = $state('');
  let checking = $state(false);

  onMount(() => {
    token = tokenStore.get();
  });

  async function login(e: Event) {
    e.preventDefault();
    error = '';
    checking = true;
    try {
      await control.perf(tokenInput); // 200 = valid token
      tokenStore.set(tokenInput);
      token = tokenInput;
      tokenInput = '';
    } catch (err) {
      error = err instanceof UnauthorizedError ? 'Invalid token.' : 'Could not reach server.';
    } finally {
      checking = false;
    }
  }

  function lock() {
    tokenStore.clear();
    token = null;
  }
</script>

<svelte:head><title>Control Panel</title></svelte:head>

{#if !token}
  <div class="min-h-screen flex items-center justify-center bg-neutral-950 text-neutral-100">
    <form onsubmit={login} class="w-80 rounded-lg border border-neutral-800 bg-neutral-900 p-6 space-y-4">
      <h1 class="text-lg font-bold flex items-center gap-2"><Lock size={18} /> Control Panel</h1>
      <input
        type="password"
        bind:value={tokenInput}
        placeholder="Admin token"
        class="w-full rounded bg-neutral-800 px-3 py-2 outline-none focus:ring-2 ring-emerald-600"
        autocomplete="off"
      />
      {#if error}<p class="text-sm text-red-400">{error}</p>{/if}
      <button
        type="submit"
        disabled={checking || !tokenInput}
        class="w-full rounded bg-emerald-600 px-3 py-2 font-semibold disabled:opacity-50 flex items-center justify-center gap-2"
      >
        <LogIn size={16} /> {checking ? 'Checking…' : 'Unlock'}
      </button>
    </form>
  </div>
{:else}
  <div class="min-h-screen bg-neutral-950 text-neutral-100">
    <header class="flex items-center justify-between border-b border-neutral-800 px-4 py-3">
      <h1 class="font-bold">Control Panel</h1>
      <button onclick={lock} class="flex items-center gap-1 text-sm text-neutral-400 hover:text-neutral-100">
        <Lock size={14} /> Lock
      </button>
    </header>
    <main class="p-4">
      <p class="text-neutral-400">Authenticated. Tabs coming in the next task.</p>
    </main>
  </div>
{/if}
```

**Step 3: Type-check + run dev.**
```bash
cd site && npm run check && npm run dev
```
Expected: check passes; dev server starts.

**Step 4: Manual browser smoke test.** With the rust-server running (same `AEVEN_ADMIN_API_TOKEN`) and the site dev server proxying `/api` to it (or testing against the deployed API), open `http://localhost:5173/control`:
- Bad token → "Invalid token."
- Correct token → shell with "Authenticated." + working Lock button.
- Reload after unlock → stays authenticated (sessionStorage); Lock → returns to gate.

> If the site dev server does not proxy `/api/*` to the rust-server, add a Vite proxy in `site/vite.config.ts` (`server.proxy['/api'] = 'http://localhost:<port>'`) for local testing only. Check the file first; do not commit a hardcoded prod URL.

**Step 5: Commit.**
```bash
git add site/src/routes/control/
git commit -m "feat(site): /control route with token auth gate"
```

---

## Task 7: Site — panel shell with tabs, auth-401 handling, auto-refresh + Overview tab

**Files:**
- Modify: `site/src/routes/control/+page.svelte`

**Step 1: Refactor the authenticated branch** into a tabbed shell with shared refresh plumbing. Replace the `{:else}` block's contents and add the supporting script state. Add to the `<script>`:
```ts
  type Tab = 'overview' | 'rooms' | 'players' | 'entities' | 'logs';
  let tab = $state<Tab>('overview');
  let autoRefresh = $state(true);
  let lastError = $state('');

  // Generic loader that bounces to the gate on 401 and keeps last-good data on other errors.
  async function load<T>(fn: () => Promise<T>, set: (v: T) => void) {
    try {
      set(await fn());
      lastError = '';
    } catch (err) {
      if (err instanceof UnauthorizedError) {
        lock();
      } else {
        lastError = err instanceof Error ? err.message : 'Request failed';
      }
    }
  }

  // ~3s poll tick. $effect re-subscribes when autoRefresh/tab/token change.
  let tick = $state(0);
  $effect(() => {
    if (!token || !autoRefresh) return;
    const id = setInterval(() => { tick++; }, 3000);
    return () => clearInterval(id);
  });
```
Import the icons you use at the top: `import { Lock, LogIn, RefreshCw, Activity } from 'lucide-svelte';` (extend as tabs are added).

**Step 2: Overview tab state + loader.** Add:
```ts
  import type { PerfSnapshot } from '$lib/control';
  let perf = $state<PerfSnapshot | null>(null);
  $effect(() => {
    if (!token || tab !== 'overview') return;
    tick; // depend on the poll tick
    load(() => control.perf(token!), (v) => (perf = v));
  });
```

**Step 3: Markup for the shell** — replace the authenticated `<main>` with a tab bar and Overview panel:
```svelte
    <nav class="flex gap-1 border-b border-neutral-800 px-4">
      {#each (['overview','rooms','players','entities','logs'] as Tab[]) as t}
        <button
          onclick={() => (tab = t)}
          class="px-3 py-2 text-sm capitalize border-b-2 {tab === t ? 'border-emerald-500 text-white' : 'border-transparent text-neutral-400 hover:text-neutral-200'}"
        >{t}</button>
      {/each}
      <div class="ml-auto flex items-center gap-3 py-2">
        <label class="flex items-center gap-1 text-xs text-neutral-400">
          <input type="checkbox" bind:checked={autoRefresh} /> Auto
        </label>
        <button onclick={() => tick++} class="text-neutral-400 hover:text-white"><RefreshCw size={14} /></button>
      </div>
    </nav>
    {#if lastError}
      <div class="bg-red-900/40 text-red-300 text-sm px-4 py-1">{lastError} (showing last data)</div>
    {/if}
    <main class="p-4">
      {#if tab === 'overview'}
        {#if perf}
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4">
            {#each [
              ['Rooms', perf.current_load.rooms],
              ['Players', perf.current_load.connected_players],
              ['Overworld', perf.current_load.overworld_players],
              ['Instances', perf.current_load.instance_players],
            ] as [label, value]}
              <div class="rounded-lg border border-neutral-800 bg-neutral-900 p-3">
                <div class="text-xs text-neutral-400">{label}</div>
                <div class="text-2xl font-bold">{value}</div>
              </div>
            {/each}
          </div>
          <h2 class="text-sm font-semibold text-neutral-300 mb-1 flex items-center gap-1"><Activity size={14} /> Recent tick spikes</h2>
          <ul class="text-xs font-mono space-y-0.5 text-neutral-400">
            {#each perf.recent_spikes.slice(0, 30) as s}<li>{s.context}</li>{/each}
          </ul>
        {:else}
          <p class="text-neutral-500">Loading…</p>
        {/if}
      {/if}
    </main>
```

**Step 4: Type-check.** `cd site && npm run check` — expect 0 errors.

**Step 5: Manual browser smoke test.** Overview shows live counts; toggling Auto stops/starts the 3s refresh; manual refresh button updates immediately; killing the server shows the red banner but keeps last data.

**Step 6: Commit.**
```bash
git add site/src/routes/control/+page.svelte
git commit -m "feat(site): control panel shell, tabs, auto-refresh, Overview"
```

---

## Task 8: Site — Rooms + Players tabs

**Files:**
- Modify: `site/src/routes/control/+page.svelte`

**Step 1: Add state + loaders** for rooms and players:
```ts
  import type { AdminRoomSummary, AdminPlayer } from '$lib/control';
  let rooms = $state<AdminRoomSummary[]>([]);
  let players = $state<AdminPlayer[]>([]);
  let playerFilter = $state('');

  $effect(() => {
    if (!token || tab !== 'rooms') return;
    tick;
    load(() => control.rooms(token!), (v) => (rooms = v));
  });
  $effect(() => {
    if (!token || tab !== 'players') return;
    tick;
    load(() => control.players(token!), (v) => (players = v));
  });

  let filteredPlayers = $derived(
    players.filter((p) =>
      !playerFilter ||
      p.name.toLowerCase().includes(playerFilter.toLowerCase()) ||
      (p.ip_address ?? '').includes(playerFilter),
    ),
  );
```

**Step 2: Rooms markup** (add a `{:else if tab === 'rooms'}` branch in `<main>`). Clicking a room opens the Entities tab scoped to it (uses `selectedRoom`, defined in Task 9 — declare it now: `let selectedRoom = $state('');`):
```svelte
      {:else if tab === 'rooms'}
        <table class="w-full text-sm">
          <thead class="text-left text-neutral-400">
            <tr><th class="py-1">Room</th><th>Players</th><th>NPCs</th><th>Overworld</th><th>Instance</th></tr>
          </thead>
          <tbody>
            {#each rooms as r}
              <tr class="border-t border-neutral-800 hover:bg-neutral-900 cursor-pointer"
                  onclick={() => { selectedRoom = r.room_id; tab = 'entities'; }}>
                <td class="py-1 font-mono">{r.room_id}</td>
                <td>{r.player_count}</td><td>{r.npc_count}</td>
                <td>{r.overworld_players}</td><td>{r.instance_players}</td>
              </tr>
            {/each}
          </tbody>
        </table>
```

**Step 3: Players markup** (`{:else if tab === 'players'}`):
```svelte
      {:else if tab === 'players'}
        <input bind:value={playerFilter} placeholder="Filter by name or IP"
               class="mb-3 w-64 rounded bg-neutral-800 px-3 py-1.5 text-sm outline-none" />
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead class="text-left text-neutral-400">
              <tr><th class="py-1">Name</th><th>Room</th><th>Instance</th><th>Pos</th><th>HP</th><th>Cmb</th><th>Flags</th><th>IP</th></tr>
            </thead>
            <tbody>
              {#each filteredPlayers as p}
                <tr class="border-t border-neutral-800">
                  <td class="py-1">{p.name}</td>
                  <td class="font-mono text-xs">{p.room_id}</td>
                  <td class="font-mono text-xs">{p.instance_id ?? '—'}</td>
                  <td class="font-mono text-xs">{p.x},{p.y},{p.z}</td>
                  <td>{p.hp}/{p.max_hp}</td>
                  <td>{p.combat_level}</td>
                  <td class="text-xs">
                    {p.is_admin ? '👑' : ''}{p.is_god_mode ? '🛡' : ''}{p.is_dead ? '💀' : ''}
                  </td>
                  <td class="font-mono text-xs">{p.ip_address ?? '—'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
```
Extend the lucide import if you add icons.

**Step 4: Type-check.** `cd site && npm run check` — expect 0 errors.

**Step 5: Manual browser smoke test.** Rooms tab lists rooms and a row-click jumps to Entities (next task wires its data); Players tab lists connected players, filter narrows by name/IP, admin flag shows for your own logged-in admin character.

**Step 6: Commit.**
```bash
git add site/src/routes/control/+page.svelte
git commit -m "feat(site): Rooms and Players tabs"
```

---

## Task 9: Site — Entities tab (per-room NPCs + players)

**Files:**
- Modify: `site/src/routes/control/+page.svelte`

**Step 1: State + loader.** `selectedRoom` was declared in Task 8. Add:
```ts
  import type { AdminRoomEntities } from '$lib/control';
  let entities = $state<AdminRoomEntities | null>(null);
  $effect(() => {
    if (!token || tab !== 'entities' || !selectedRoom) return;
    tick;
    load(() => control.roomEntities(token!, selectedRoom), (v) => (entities = v));
  });
```

**Step 2: Markup** (`{:else if tab === 'entities'}`). Includes a room picker (defaults from the Rooms list) so the tab is usable even without clicking through:
```svelte
      {:else if tab === 'entities'}
        <div class="mb-3 flex items-center gap-2">
          <label class="text-sm text-neutral-400">Room</label>
          <select bind:value={selectedRoom} class="rounded bg-neutral-800 px-2 py-1 text-sm">
            <option value="" disabled>Select a room…</option>
            {#each rooms as r}<option value={r.room_id}>{r.room_id}</option>{/each}
          </select>
        </div>
        {#if entities}
          <h2 class="text-sm font-semibold text-neutral-300 mb-1">NPCs ({entities.npcs.length})</h2>
          <div class="overflow-x-auto mb-4">
            <table class="w-full text-sm">
              <thead class="text-left text-neutral-400">
                <tr><th class="py-1">Name</th><th>Proto</th><th>Pos</th><th>HP</th><th>Lv</th><th>State</th><th>Target</th></tr>
              </thead>
              <tbody>
                {#each entities.npcs as n}
                  <tr class="border-t border-neutral-800 {n.hidden ? 'opacity-50' : ''}">
                    <td class="py-1">{n.display_name}</td>
                    <td class="font-mono text-xs">{n.prototype_id}</td>
                    <td class="font-mono text-xs">{n.x},{n.y},{n.z}</td>
                    <td>{n.hp}/{n.max_hp}</td><td>{n.level}</td>
                    <td>{n.state}{n.invulnerable ? ' 🛡' : ''}</td>
                    <td class="font-mono text-xs">{n.target_id ?? '—'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
          <h2 class="text-sm font-semibold text-neutral-300 mb-1">Players ({entities.players.length})</h2>
          <ul class="text-sm space-y-0.5">
            {#each entities.players as p}
              <li class="font-mono text-xs">{p.name} @ {p.x},{p.y},{p.z} — {p.hp}/{p.max_hp} hp</li>
            {/each}
          </ul>
        {:else if selectedRoom}
          <p class="text-neutral-500">Loading…</p>
        {:else}
          <p class="text-neutral-500">Pick a room to inspect.</p>
        {/if}
```
> The room picker needs the `rooms` list populated. Add a one-shot fetch when entering the tab with an empty list:
```ts
  $effect(() => {
    if (!token || tab !== 'entities' || rooms.length) return;
    load(() => control.rooms(token!), (v) => (rooms = v));
  });
```

**Step 3: Type-check.** `cd site && npm run check` — expect 0 errors.

**Step 4: Manual browser smoke test.** From Rooms, click a room → Entities shows its NPCs (state, target, hidden dimmed) and players; the room `<select>` switches rooms; hidden/invulnerable NPCs render distinctly.

**Step 5: Commit.**
```bash
git add site/src/routes/control/+page.svelte
git commit -m "feat(site): Entities tab (per-room NPCs and players)"
```

---

## Task 10: Site — Logs tab

**Files:**
- Modify: `site/src/routes/control/+page.svelte`

**Step 1: State + loader.**
```ts
  import type { LogEntry } from '$lib/control';
  let logs = $state<LogEntry[]>([]);
  let logLevel = $state('');         // '' = all
  let logImportant = $state(false);
  let logCount = $state(200);
  $effect(() => {
    if (!token || tab !== 'logs') return;
    tick;
    const opts = { count: logCount, level: logLevel || undefined, important: logImportant };
    load(() => control.logs(token!, opts), (v) => (logs = v));
  });
```

**Step 2: Markup** (`{:else if tab === 'logs'}`). Color rows by level:
```svelte
      {:else if tab === 'logs'}
        <div class="mb-3 flex flex-wrap items-center gap-3 text-sm">
          <select bind:value={logLevel} class="rounded bg-neutral-800 px-2 py-1">
            <option value="">All levels</option>
            <option value="ERROR">Error</option>
            <option value="WARN">Warn</option>
            <option value="INFO">Info</option>
          </select>
          <label class="flex items-center gap-1 text-neutral-400">
            <input type="checkbox" bind:checked={logImportant} /> Important only
          </label>
          <select bind:value={logCount} class="rounded bg-neutral-800 px-2 py-1">
            {#each [100, 200, 500, 1000] as c}<option value={c}>{c} lines</option>{/each}
          </select>
        </div>
        <div class="font-mono text-xs space-y-0.5 max-h-[70vh] overflow-y-auto">
          {#each logs as l}
            <div class="{l.level === 'ERROR' ? 'text-red-400' : l.level === 'WARN' ? 'text-amber-400' : 'text-neutral-300'}">
              <span class="text-neutral-500">{l.timestamp ?? ''}</span>
              <span class="font-bold">[{l.level}]</span> {l.message}
            </div>
          {/each}
        </div>
```
> Confirm the real field names of a log entry against `/api/logs` output during the smoke test (`level`, `message`, `timestamp`); adjust the markup and `LogEntry` interface in `control.ts` if they differ (e.g. `msg`, `time`).

**Step 3: Type-check.** `cd site && npm run check` — expect 0 errors.

**Step 4: Manual browser smoke test.** Logs tab shows recent entries; level filter and "Important only" and count selector all change the result; ERROR rows render red, WARN amber.

**Step 5: Commit.**
```bash
git add site/src/routes/control/+page.svelte
git commit -m "feat(site): Logs tab"
```

---

## Task 11: Full verification pass

**Step 1: Server — build + tests.**
```bash
cd rust-server && cargo build -p isometric-server && cargo test -p isometric-server admin_api
```
Expected: builds; `npc_state_label_covers_all_variants` passes.

**Step 2: Site — type check + production build** (catches prerender/adapter issues for the new route).
```bash
cd site && npm run check && npm run build
```
Expected: 0 type errors; build succeeds (the `/control` route is client-only via `ssr=false`/`prerender=false`).

**Step 3: End-to-end manual pass** with server + site running and a game client logged in:
- Gate rejects a bad token, accepts the real one, persists across reload, Lock returns to gate.
- All five tabs load live data; auto-refresh ticks ~3s; manual refresh works.
- Rooms → click → Entities scoped correctly.
- Players filter works; IP column populated.
- Stopping the server shows the red banner without wiping the view; a 401 (e.g. wrong token mid-session) bounces to the gate.

**Step 4: Use superpowers:requesting-code-review** to review the branch before integration.

**Step 5: Use superpowers:finishing-a-development-branch** to decide merge/PR/cleanup.

---

## Notes / decisions captured

- **No CORS changes:** same-origin in prod (`/api/*` reverse-proxied). Local dev may need a temporary Vite proxy (Task 6 note) — do not commit a hardcoded API URL.
- **IP addresses are shown** (design decision) — useful for spotting multi-logging/ban-evasion. The panel is bearer-gated; treat the token as a secret.
- **Read-only:** no kick/ban/teleport actions from the panel (future work).
- **Polling, not streaming:** 3s interval; no WebSocket/SSE (future work if needed).
- **Room key:** endpoints iterate all rooms regardless of key name; the overworld key is whatever `GameRoom.id` is (commonly `overworld`).
