<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Item } from '$lib/api';
  import { Gem, Search } from 'lucide-svelte';

  const CATEGORIES = ['all', 'equipment', 'consumable', 'material', 'quest'] as const;

  const categoryBadge: Record<string, string> = {
    equipment: 'bg-[var(--water)]/20 text-[var(--water)]',
    consumable: 'bg-[var(--moss)]/20 text-[var(--moss-light)]',
    material: 'bg-[var(--gold)]/20 text-[var(--gold)]',
    quest: 'bg-[var(--ember)]/20 text-[var(--ember)]',
  };

  let data: Item[] | undefined = $state();
  let isLoading = $state(true);
  let search = $state('');
  let category = $state<string>('all');

  let filtered = $derived.by(() => {
    if (!data) return [];
    const q = search.toLowerCase();
    return [...data]
      .filter((item) => {
        if (category !== 'all' && item.category !== category) return false;
        if (q && !item.display_name.toLowerCase().includes(q) && !item.id.toLowerCase().includes(q)) return false;
        return true;
      })
      .sort((a, b) => a.display_name.localeCompare(b.display_name));
  });

  onMount(async () => {
    document.title = 'Item Registry — New Aeven World Statistics';
    try {
      data = await api.items();
    } finally {
      isLoading = false;
    }
  });
</script>

<svelte:head>
  <title>Item Registry — New Aeven World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <div class="flex items-center gap-3">
    <Gem size={22} class="text-[var(--gold)]" />
    <h1 class="text-2xl font-bold text-[var(--text)]">Item Registry</h1>
    {#if data}
      <span class="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">{filtered.length}</span>
    {/if}
  </div>

  <div class="relative w-full max-w-sm">
    <Search size={14} class="absolute top-1/2 left-3 -translate-y-1/2 text-[var(--muted)]" />
    <input
      type="text"
      placeholder="Search items..."
      bind:value={search}
      class="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] py-2 pr-4 pl-9 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none transition-colors focus:border-[var(--gold)]"
    />
  </div>

  <div class="flex flex-wrap gap-2">
    {#each CATEGORIES as cat}
      <button
        onclick={() => (category = cat)}
        class="pixel-btn rounded-md px-4 py-1.5 text-xs font-bold transition-colors {category === cat
          ? 'bg-[var(--gold)] text-[#1a1210]'
          : 'bg-[var(--panel)] text-[var(--text-soft)] hover:text-[var(--text)]'}"
      >
        {cat.charAt(0).toUpperCase() + cat.slice(1)}
      </button>
    {/each}
  </div>

  {#if isLoading}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each Array(8) as _, i}
        <div class="space-y-3 rounded-lg border border-[var(--panel-border)] bg-[var(--panel)] p-4">
          <div class="h-5 w-32 animate-pulse rounded bg-[var(--panel-soft)]"></div>
          <div class="h-4 w-20 animate-pulse rounded bg-[var(--panel-soft)]"></div>
          <div class="h-4 w-full animate-pulse rounded bg-[var(--panel-soft)]"></div>
        </div>
      {/each}
    </div>
  {:else if filtered.length === 0}
    <p class="py-12 text-center text-[var(--muted)]">No items found</p>
  {:else}
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each filtered as item}
        <div class="pixel-box space-y-2 rounded-lg bg-[var(--panel)] p-4 transition-colors hover:border-[var(--gold)]/40">
          <p class="font-bold text-[var(--text)]">{item.display_name}</p>
          <div class="flex flex-wrap items-center gap-2">
            <span class="rounded-full px-2 py-0.5 text-xs font-medium {categoryBadge[item.category] ?? 'bg-[var(--panel-soft)] text-[var(--muted)]'}">{item.category}</span>
            {#if item.equipment}
              <span class="rounded-full bg-[var(--panel-soft)] px-2 py-0.5 text-xs text-[var(--muted)]">{item.equipment.slot_type}</span>
            {/if}
          </div>
          {#if item.description}
            <p class="line-clamp-2 text-sm text-[var(--text-soft)]">{item.description}</p>
          {/if}
          {#if item.base_price > 0}
            <p class="text-sm text-[var(--gold)]">{item.base_price.toLocaleString()} gold</p>
          {/if}
          {#if item.equipment}
            {@const eq = item.equipment}
            <div class="space-y-1 border-t border-[var(--panel-border)] pt-1">
              <div class="flex flex-wrap gap-x-3 gap-y-0.5">
                {#if eq.attack_bonus}
                  <span class="text-xs {eq.attack_bonus > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'}">{eq.attack_bonus > 0 ? '+' : ''}{eq.attack_bonus} Attack</span>
                {/if}
                {#if eq.strength_bonus}
                  <span class="text-xs {eq.strength_bonus > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'}">{eq.strength_bonus > 0 ? '+' : ''}{eq.strength_bonus} Strength</span>
                {/if}
                {#if eq.defence_bonus}
                  <span class="text-xs {eq.defence_bonus > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'}">{eq.defence_bonus > 0 ? '+' : ''}{eq.defence_bonus} Defence</span>
                {/if}
                {#if eq.magic_bonus}
                  <span class="text-xs {eq.magic_bonus > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'}">{eq.magic_bonus > 0 ? '+' : ''}{eq.magic_bonus} Magic</span>
                {/if}
                {#if eq.ranged_strength_bonus}
                  <span class="text-xs {eq.ranged_strength_bonus > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'}">{eq.ranged_strength_bonus > 0 ? '+' : ''}{eq.ranged_strength_bonus} Ranged Str</span>
                {/if}
              </div>
              <div class="flex flex-wrap gap-x-3 gap-y-0.5">
                {#if eq.ranged_level_required > 0}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.ranged_level_required} Ranged</span>
                {:else if eq.attack_level_required > 1}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.attack_level_required} Attack</span>
                {/if}
                {#if eq.defence_level_required > 1}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.defence_level_required} Defence</span>
                {/if}
                {#if eq.woodcutting_level_required > 1}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.woodcutting_level_required} Woodcutting</span>
                {/if}
                {#if eq.mining_level_required > 1}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.mining_level_required} Mining</span>
                {/if}
                {#if eq.magic_level_required > 0}
                  <span class="text-xs text-[var(--gold)]">Requires {eq.magic_level_required} Magic</span>
                {/if}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
