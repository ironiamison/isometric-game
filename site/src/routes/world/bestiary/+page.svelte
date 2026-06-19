<script lang="ts">
  import EntitySprite from '$lib/components/EntitySprite.svelte';
  import type { Entity } from '$lib/api';
  import { ChevronLeft, ChevronRight, Search, Skull } from '@lucide/svelte';
  import type { PageData } from './$types';

  const PAGE_SIZE = 48;

  let { data }: { data: PageData } = $props();

  let search = $state('');
  let page = $state(1);
  let entities = $derived(data.entities as Entity[]);

  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return entities
      .filter((e) => !q || e.display_name.toLowerCase().includes(q) || e.id.toLowerCase().includes(q))
      .sort((a, b) => a.level - b.level || a.display_name.localeCompare(b.display_name));
  });

  let totalPages = $derived(Math.max(1, Math.ceil(filtered.length / PAGE_SIZE)));
  let pageEntities = $derived(filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE));

  function onSearchChange() {
    page = 1;
  }
</script>

<svelte:head>
  <title>Bestiary — Solstead World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <section class="bestiary-hero pixel-box overflow-hidden rounded-xl">
    <div class="bestiary-hero-bg" aria-hidden="true"></div>
    <div class="bestiary-hero-shade" aria-hidden="true"></div>
    <div class="bestiary-hero-content">
      <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
        <Skull size={14} class="text-[var(--ember)]" />
        Field Guide
      </p>
      <h1 class="mt-2 text-3xl font-bold text-[var(--text)] md:text-4xl">Bestiary</h1>
      <p class="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">
        Every monster in Solstead. Stats, drops, and scaling — know your enemy.
      </p>
    </div>
  </section>

  <div class="flex flex-wrap items-center gap-3">
    <div class="relative w-full max-w-sm">
      <Search size={14} class="absolute top-1/2 left-3 -translate-y-1/2 text-[var(--muted)]" />
      <input
        type="search"
        placeholder="Search monsters..."
        bind:value={search}
        oninput={onSearchChange}
        class="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] py-2 pr-4 pl-9 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none transition-colors focus:border-[var(--gold)]"
      />
    </div>
    <span class="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">{filtered.length}</span>
  </div>

  {#if filtered.length === 0}
    <p class="py-12 text-center text-[var(--muted)]">No monsters found</p>
  {:else}
    {#key `${search}-${page}`}
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {#each pageEntities as entity (entity.id)}
          <a
            href="/world/bestiary/{encodeURIComponent(entity.id)}"
            class="pixel-box flex gap-3 rounded-lg bg-[var(--panel)] p-4 transition-colors hover:border-[var(--gold)]/40"
          >
            <EntitySprite sprite={entity.sprite} alt={entity.display_name} />
            <div class="min-w-0 flex-1 space-y-2">
              <div class="flex items-center justify-between gap-2">
                <p class="font-bold text-[var(--text)]">{entity.display_name}</p>
                <span class="shrink-0 rounded-full bg-[var(--panel-soft)] px-2 py-0.5 font-mono text-xs text-[var(--muted)]">Lv {entity.level}</span>
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
            </div>
          </a>
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
  .bestiary-hero {
    position: relative;
    min-height: 160px;
  }

  .bestiary-hero-bg {
    position: absolute;
    inset: 0;
    background: url('/world/bestiary-hero.png') center 35% / cover no-repeat;
  }

  .bestiary-hero-shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(14, 10, 8, 0.92) 0%, rgba(14, 10, 8, 0.72) 45%, rgba(14, 10, 8, 0.35) 100%),
      linear-gradient(180deg, rgba(180, 60, 60, 0.12), transparent 55%);
  }

  .bestiary-hero-content {
    position: relative;
    z-index: 1;
    padding: 28px 24px 32px;
  }

  @media (min-width: 768px) {
    .bestiary-hero {
      min-height: 180px;
    }

    .bestiary-hero-content {
      padding: 32px 32px 36px;
    }
  }
</style>
