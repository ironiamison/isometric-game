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
