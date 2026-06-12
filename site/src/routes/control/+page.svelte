<script lang="ts">
  import { onMount } from 'svelte';
  import { control, tokenStore, UnauthorizedError } from '$lib/control';
  import type { PerfSnapshot } from '$lib/control';
  import { Lock, LogIn, RefreshCw, Activity } from 'lucide-svelte';

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
      {/if}
    </main>
  </div>
{/if}
