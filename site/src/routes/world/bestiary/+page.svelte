<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Entity } from '$lib/api';
  import { Search, Skull } from 'lucide-svelte';

  let data: Entity[] | undefined = $state();
  let isLoading = $state(true);
  let search = $state('');

  let filtered = $derived.by(() => {
    if (!data) return [];
    const q = search.toLowerCase();
    return [...data]
      .filter((e) => !q || e.display_name.toLowerCase().includes(q) || e.id.toLowerCase().includes(q))
      .sort((a, b) => a.level - b.level || a.display_name.localeCompare(b.display_name));
  });

  onMount(async () => {
    document.title = 'Bestiary — New Aeven World Statistics';
    try {
      data = await api.entities();
    } finally {
      isLoading = false;
    }
  });
</script>

<svelte:head>
  <title>Bestiary — New Aeven World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <section class="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_20%_15%,rgba(180,60,60,0.18),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(212,168,68,0.14),transparent_45%),var(--panel)] px-6 py-7 md:px-8">
    <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
      <Skull size={14} class="text-[var(--ember)]" />
      Field Guide
    </p>
    <h1 class="mt-2 text-3xl font-bold text-[var(--text)] md:text-4xl">Bestiary</h1>
    <p class="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">Every monster in New Aeven. Stats, drops, and scaling — know your enemy.</p>
  </section>

  <div class="flex items-center gap-3">
    <div class="relative w-full max-w-sm">
      <Search size={14} class="absolute top-1/2 left-3 -translate-y-1/2 text-[var(--muted)]" />
      <input
        type="text"
        placeholder="Search monsters..."
        bind:value={search}
        class="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] py-2 pr-4 pl-9 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none transition-colors focus:border-[var(--gold)]"
      />
    </div>
    {#if data}
      <span class="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">{filtered.length}</span>
    {/if}
  </div>

  {#if isLoading}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each Array(8) as _, i}
        <div class="space-y-3 rounded-lg border border-[var(--panel-border)] bg-[var(--panel)] p-4">
          <div class="h-5 w-32 animate-pulse rounded bg-[var(--panel-soft)]"></div>
          <div class="h-4 w-20 animate-pulse rounded bg-[var(--panel-soft)]"></div>
        </div>
      {/each}
    </div>
  {:else if filtered.length === 0}
    <p class="py-12 text-center text-[var(--muted)]">No monsters found</p>
  {:else}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each filtered as entity}
        <a href="/world/bestiary/{encodeURIComponent(entity.id)}" class="pixel-box block space-y-2 rounded-lg bg-[var(--panel)] p-4 transition-colors hover:border-[var(--gold)]/40">
          <div class="flex items-center justify-between">
            <p class="font-bold text-[var(--text)]">{entity.display_name}</p>
            <span class="rounded-full bg-[var(--panel-soft)] px-2 py-0.5 font-mono text-xs text-[var(--muted)]">Lv {entity.level}</span>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <span class="rounded-full px-2 py-0.5 text-xs font-medium {entity.hostile ? 'bg-[var(--ember)]/20 text-[var(--ember)]' : 'bg-[var(--moss)]/20 text-[var(--moss-light)]'}">
              {entity.hostile ? 'Aggressive' : 'Passive'}
            </span>
            {#if entity.loot.length > 0}
              <span class="rounded-full bg-[var(--gold)]/15 px-2 py-0.5 text-xs text-[var(--gold)]">{entity.loot.length} drops</span>
            {/if}
          </div>
          {#if entity.description}
            <p class="line-clamp-2 text-sm text-[var(--text-soft)]">{entity.description}</p>
          {/if}
          <div class="flex flex-wrap gap-x-4 gap-y-1 border-t border-[var(--panel-border)] pt-1 text-xs">
            <span class="text-[var(--text-soft)]">HP <span class="font-mono text-[var(--text)]">{entity.max_hp}</span></span>
            <span class="text-[var(--text-soft)]">Dmg <span class="font-mono text-[var(--text)]">{entity.damage}</span></span>
            <span class="text-[var(--text-soft)]">XP <span class="font-mono text-[var(--text)]">{entity.exp_base * entity.level}</span></span>
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>
