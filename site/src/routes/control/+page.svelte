<script lang="ts">
  import { onMount } from 'svelte';
  import { control, tokenStore, UnauthorizedError } from '$lib/control';
  import type { PerfSnapshot, AdminRoomSummary, AdminPlayer, AdminRoomEntities, LogEntry } from '$lib/control';
  import { Lock, LogIn, RefreshCw, Activity } from '@lucide/svelte';

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

  // Overview tab state + loader.
  let perf = $state<PerfSnapshot | null>(null);
  $effect(() => {
    if (!token || tab !== 'overview') return;
    tick; // depend on the poll tick
    load(() => control.perf(token!), (v) => (perf = v));
  });

  // Rooms + Players tab state + loaders.
  let rooms = $state<AdminRoomSummary[]>([]);
  let players = $state<AdminPlayer[]>([]);
  let playerFilter = $state('');
  let selectedRoom = $state('');

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

  // Entities tab state + loaders.
  let entities = $state<AdminRoomEntities | null>(null);
  $effect(() => {
    if (!token || tab !== 'entities' || !selectedRoom) return;
    tick;
    load(() => control.roomEntities(token!, selectedRoom), (v) => (entities = v));
  });
  // Ensure the room picker has options when entering the tab directly.
  $effect(() => {
    if (!token || tab !== 'entities' || rooms.length) return;
    load(() => control.rooms(token!), (v) => (rooms = v));
  });

  // Logs tab state + loader.
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
      {:else if tab === 'entities'}
        <div class="mb-3 flex items-center gap-2">
          <label class="text-sm text-neutral-400" for="room-select">Room</label>
          <select id="room-select" bind:value={selectedRoom} class="rounded bg-neutral-800 px-2 py-1 text-sm">
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
      {/if}
    </main>
  </div>
{/if}
