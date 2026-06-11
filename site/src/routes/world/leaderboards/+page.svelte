<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LeaderboardEntry, type LeaderboardSort } from '$lib/api';
  import { formatPlayedTime } from '$lib/format';
  import { METRIC_GROUPS, METRICS, metricValue, type Metric } from '$lib/leaderboard';
  import { Search, Trophy } from 'lucide-svelte';

  let activeSort: LeaderboardSort = $state('total_level');
  let search = $state('');
  let data: LeaderboardEntry[] | undefined = $state();
  let isLoading = $state(true);

  let metric: Metric = $derived(METRICS.find((m) => m.sort === activeSort) ?? METRICS[0]);
  let ranked = $derived((data ?? []).map((entry, index) => ({ rank: index + 1, entry })));
  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return ranked;
    return ranked.filter((item) => item.entry.name.toLowerCase().includes(q));
  });
  let champions = $derived(ranked.slice(0, 3));

  function rankStyle(rank: number) {
    if (rank === 1) return 'border-[var(--gold)] bg-[var(--gold)]/10';
    if (rank === 2) return 'border-[#9ca3af] bg-[#9ca3af]/10';
    if (rank === 3) return 'border-[#ad7b46] bg-[#ad7b46]/10';
    return 'border-[var(--panel-border)] bg-[var(--panel-soft)]';
  }

  async function load() {
    isLoading = true;
    try {
      data = await api.leaderboard(activeSort, 200);
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    activeSort;
    load();
  });

  onMount(() => {
    document.title = 'Leaderboards — New Aeven World Statistics';
  });
</script>

<svelte:head>
  <title>Leaderboards — New Aeven World Statistics</title>
</svelte:head>

<div class="space-y-6">
  <section class="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_20%_15%,rgba(212,168,68,0.22),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(90,114,71,0.16),transparent_45%),var(--panel)] px-6 py-7 md:px-8">
    <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
      <Trophy size={14} class="text-[var(--gold)]" />
      Hall Of Legends
    </p>
    <h1 class="mt-2 text-3xl font-bold text-[var(--text)] md:text-4xl">Player Leaderboards</h1>
    <p class="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">Every skill. Every record. Find your name — or someone to beat.</p>
  </section>

  <section class="grid gap-4 md:grid-cols-3">
    {#if isLoading}
      {#each Array(3) as _, i}
        <div class="h-32 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]"></div>
      {/each}
    {:else}
      {#each champions as { rank, entry }}
        <div class="pixel-box rounded-xl p-4 transition-colors {rankStyle(rank)}">
          <p class="text-xs tracking-[0.18em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">Rank {rank}</p>
          <a href="/world/player/{encodeURIComponent(entry.name)}" class="mt-2 block text-xl font-bold text-[var(--text)] hover:text-[var(--gold)]">{entry.name}</a>
          <p class="mt-1 text-sm text-[var(--text-soft)]">{metricValue(metric, entry)} {metric.label}</p>
        </div>
      {/each}
    {/if}
  </section>

  <section class="pixel-box space-y-4 rounded-xl bg-[var(--panel)] p-4 md:p-5">
    {#each METRIC_GROUPS as group}
      <div>
        <p class="mb-2 text-[11px] tracking-[0.2em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">{group.title}</p>
        <div class="flex flex-wrap gap-2">
          {#each group.metrics as item}
            <button
              onclick={() => (activeSort = item.sort)}
              class="pixel-btn rounded-md px-3 py-1.5 text-xs font-bold transition-colors {item.sort === metric.sort
                ? 'bg-[var(--gold)] text-[#1a1210]'
                : 'bg-[var(--panel-soft)] text-[var(--text-soft)] hover:text-[var(--text)]'}"
            >
              {item.label}
            </button>
          {/each}
        </div>
      </div>
    {/each}

    <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <p class="text-sm text-[var(--text-soft)]">
        Active board: <span class="text-[var(--text)]">{metric.label}</span> — {metric.hint}
      </p>
      <div class="relative w-full md:max-w-xs">
        <Search size={14} class="absolute top-1/2 left-3 -translate-y-1/2 text-[var(--muted)]" />
        <input
          type="text"
          placeholder="Search player..."
          bind:value={search}
          class="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] py-2 pr-3 pl-9 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none transition-colors focus:border-[var(--gold)]"
        />
      </div>
    </div>

    <div class="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
      <table class="w-full min-w-[720px]">
        <thead>
          <tr class="bg-[var(--panel-soft)]">
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Rank</th>
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Player</th>
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">{metric.label}</th>
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Total Level</th>
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Monster Kills</th>
            <th class="px-4 py-3 text-left text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase">Played</th>
          </tr>
        </thead>
        <tbody>
          {#if isLoading}
            {#each Array(10) as _, i}
              <tr class="border-t border-[var(--panel-border)]">
                {#each Array(6) as __}
                  <td class="px-4 py-3"><div class="h-3.5 w-16 animate-pulse rounded bg-[var(--panel-soft)]"></div></td>
                {/each}
              </tr>
            {/each}
          {:else if filtered.length === 0}
            <tr>
              <td colspan={6} class="px-4 py-10 text-center text-[var(--text-soft)]">No players matched that search.</td>
            </tr>
          {:else}
            {#each filtered as { rank, entry }}
              <tr class="border-t border-[var(--panel-border)] hover:bg-[var(--panel-soft)]/70">
                <td class="px-4 py-3 font-mono text-sm text-[var(--text-soft)]">{rank}</td>
                <td class="px-4 py-3 font-medium">
                  <a href="/world/player/{encodeURIComponent(entry.name)}" class="text-[var(--text)] hover:text-[var(--gold)]">{entry.name}</a>
                </td>
                <td class="px-4 py-3 font-mono text-[var(--text)]">{metricValue(metric, entry)}</td>
                <td class="px-4 py-3 font-mono text-[var(--text-soft)]">{entry.total_level.toLocaleString()}</td>
                <td class="px-4 py-3 font-mono text-[var(--text-soft)]">{entry.monster_kills.toLocaleString()}</td>
                <td class="px-4 py-3 text-[var(--text-soft)]">{formatPlayedTime(entry.played_time)}</td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  </section>
</div>
