<script lang="ts">
  import ItemSprite from '$lib/components/ItemSprite.svelte';
  import type { Item } from '$lib/api';
  import { ChevronLeft, ChevronRight, Gem, Search } from '@lucide/svelte';
  import type { PageData } from './$types';

  const CATEGORIES = ['all', 'equipment', 'consumable', 'material', 'quest'] as const;
  const PAGE_SIZE = 48;

  const categoryBadge: Record<string, string> = {
    equipment: 'bg-[var(--water)]/20 text-[var(--water)]',
    consumable: 'bg-[var(--moss)]/20 text-[var(--moss-light)]',
    material: 'bg-[var(--gold)]/20 text-[var(--gold)]',
    quest: 'bg-[var(--ember)]/20 text-[var(--ember)]',
  };

  let { data }: { data: PageData } = $props();

  let search = $state('');
  let category = $state<(typeof CATEGORIES)[number]>('all');
  let page = $state(1);

  let items = $derived(data.items as Item[]);

  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return items
      .filter((item) => {
        if (category !== 'all' && item.category !== category) return false;
        if (q && !item.display_name.toLowerCase().includes(q) && !item.id.toLowerCase().includes(q)) return false;
        return true;
      })
      .sort((a, b) => a.display_name.localeCompare(b.display_name));
  });

  let categoryCounts = $derived.by(() => {
    const counts: Record<string, number> = { all: items.length };
    for (const item of items) {
      counts[item.category] = (counts[item.category] ?? 0) + 1;
    }
    return counts;
  });

  let totalPages = $derived(Math.max(1, Math.ceil(filtered.length / PAGE_SIZE)));
  let pageItems = $derived(filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE));
  let showingFrom = $derived(filtered.length === 0 ? 0 : (page - 1) * PAGE_SIZE + 1);
  let showingTo = $derived(Math.min(page * PAGE_SIZE, filtered.length));

  function selectCategory(cat: (typeof CATEGORIES)[number]) {
    category = cat;
    page = 1;
  }

  function onSearchChange() {
    page = 1;
  }
</script>

<svelte:head>
  <title>Item Registry — Solstead World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <section class="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_20%_15%,rgba(212,168,68,0.16),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(90,138,170,0.12),transparent_45%),var(--panel)] px-6 py-7 md:px-8">
    <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
      <Gem size={14} class="text-[var(--gold)]" />
      Item Registry
    </p>
    <h1 class="mt-2 text-3xl font-bold text-[var(--text)] md:text-4xl">Items</h1>
    <p class="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">
      Every item in Solstead — {items.length} entries from game data. Weapons, armour, tools, consumables, and materials.
    </p>
  </section>

  <div class="flex flex-wrap items-center gap-3">
    <div class="relative w-full max-w-sm">
      <Search size={14} class="absolute top-1/2 left-3 -translate-y-1/2 text-[var(--muted)]" />
      <input
        type="search"
        placeholder="Search items..."
        bind:value={search}
        oninput={onSearchChange}
        class="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] py-2 pr-4 pl-9 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none transition-colors focus:border-[var(--gold)]"
      />
    </div>
    <span class="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">{filtered.length}</span>
  </div>

  <div class="flex flex-wrap gap-2" role="tablist" aria-label="Item categories">
    {#each CATEGORIES as cat (cat)}
      <button
        type="button"
        role="tab"
        aria-selected={category === cat}
        onclick={() => selectCategory(cat)}
        class="category-btn pixel-btn rounded-md px-4 py-1.5 text-xs font-bold transition-colors {category === cat
          ? 'is-active'
          : ''}"
      >
        {cat.charAt(0).toUpperCase() + cat.slice(1)}
        <span class="ml-1 opacity-75">({categoryCounts[cat] ?? 0})</span>
      </button>
    {/each}
  </div>

  {#if filtered.length === 0}
    <p class="py-12 text-center text-[var(--muted)]">No items found</p>
  {:else}
    <p class="text-sm text-[var(--muted)]">
      Showing {showingFrom}–{showingTo} of {filtered.length}
      {#if category !== 'all' || search.trim()}
        <span class="text-[var(--text-soft)]"> (filtered from {items.length} total)</span>
      {/if}
    </p>
    {#key `${category}-${search}-${page}`}
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {#each pageItems as item (item.id)}
          <article class="pixel-box flex gap-3 rounded-lg bg-[var(--panel)] p-4 transition-colors hover:border-[var(--gold)]/40">
            <ItemSprite sprite={item.sprite} alt={item.display_name} />
            <div class="min-w-0 flex-1 space-y-2">
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
          </article>
        {/each}
      </div>
    {/key}

    {#if totalPages > 1}
      <div class="flex items-center justify-center gap-3 pt-2">
        <button
          type="button"
          class="pixel-btn inline-flex items-center gap-1 rounded-md bg-[var(--panel)] px-3 py-2 text-xs font-bold text-[var(--text-soft)] disabled:opacity-40"
          disabled={page <= 1}
          onclick={() => (page = Math.max(1, page - 1))}
        >
          <ChevronLeft size={14} />
          Prev
        </button>
        <span class="font-mono text-sm text-[var(--text-soft)]">Page {page} / {totalPages}</span>
        <button
          type="button"
          class="pixel-btn inline-flex items-center gap-1 rounded-md bg-[var(--panel)] px-3 py-2 text-xs font-bold text-[var(--text-soft)] disabled:opacity-40"
          disabled={page >= totalPages}
          onclick={() => (page = Math.min(totalPages, page + 1))}
        >
          Next
          <ChevronRight size={14} />
        </button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .category-btn {
    background: var(--panel);
    color: var(--text-soft);
    border: 1px solid var(--panel-border);
    cursor: pointer;
  }

  .category-btn:hover {
    color: var(--text);
    border-color: color-mix(in oklab, var(--gold) 40%, transparent);
  }

  .category-btn.is-active {
    background: var(--gold);
    color: #1a1210;
    border-color: var(--gold);
  }
</style>
