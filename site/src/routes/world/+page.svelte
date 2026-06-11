<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LeaderboardEntry, type Overview } from '$lib/api';
  import { Crown, Crosshair, Gem, Signal, Skull, Trophy, UserCheck, UserRound, Users } from 'lucide-svelte';

  let overview: Overview | undefined = $state();
  let topLevels: LeaderboardEntry[] | undefined = $state();
  let topHunters: LeaderboardEntry[] | undefined = $state();
  let loadingOverview = $state(true);
  let loadingTopLevels = $state(true);
  let loadingTopHunters = $state(true);

  async function load() {
    try {
      const [o, levels, hunters] = await Promise.all([
        api.overview(),
        api.leaderboard('total_level', 5),
        api.leaderboard('monster_kills', 5),
      ]);
      overview = o;
      topLevels = levels;
      topHunters = hunters;
    } finally {
      loadingOverview = false;
      loadingTopLevels = false;
      loadingTopHunters = false;
    }
  }

  onMount(() => {
    document.title = 'World Pulse — New Aeven World Statistics';
    load();
    const id = setInterval(load, 15_000);
    return () => clearInterval(id);
  });
</script>

<svelte:head>
  <title>World Pulse — New Aeven World Statistics</title>
  <meta
    name="description"
    content="Live world statistics for New Aeven — track online players, leaderboards, player rankings, and browse the full item registry."
  />
  <link rel="canonical" href="https://aeven.xyz/world/" />
</svelte:head>

<div class="space-y-5">
  <h1 class="text-3xl font-bold text-[var(--text)] md:text-4xl">New Aeven Stats</h1>

  <div class="grid grid-cols-3 gap-2 md:grid-cols-6 md:gap-3">
    <div class="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
      <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
        <Signal size={10} class="text-[var(--moss-light)]" />
        Online
      </span>
      {#if loadingOverview}
        <div class="mt-1.5 h-6 w-10 animate-pulse rounded bg-[var(--panel-soft)]"></div>
      {:else}
        <p class="mt-1.5 text-xl font-bold text-[var(--gold)]">{(overview?.online_players ?? 0).toLocaleString()}</p>
      {/if}
    </div>

    <div class="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
      <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
        <UserRound size={10} />
        Characters
      </span>
      {#if loadingOverview}
        <div class="mt-1.5 h-6 w-12 animate-pulse rounded bg-[var(--panel-soft)]"></div>
      {:else}
        <p class="mt-1.5 text-xl font-bold text-[var(--text)]">{(overview?.total_characters ?? 0).toLocaleString()}</p>
      {/if}
    </div>

    <div class="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
      <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
        <UserCheck size={10} />
        Accounts
      </span>
      {#if loadingOverview}
        <div class="mt-1.5 h-6 w-12 animate-pulse rounded bg-[var(--panel-soft)]"></div>
      {:else}
        <p class="mt-1.5 text-xl font-bold text-[var(--text)]">{(overview?.total_accounts ?? 0).toLocaleString()}</p>
      {/if}
    </div>

    <a href="/world/players" class="pixel-box group col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5 transition-colors hover:border-[var(--gold)]/50">
      <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
        <Users size={10} />
        Live Players
      </span>
      <p class="mt-1.5 text-xs font-semibold text-[var(--gold)] opacity-60 transition-opacity group-hover:opacity-100">View &rarr;</p>
    </a>

    <a href="/world/leaderboards" class="pixel-box group col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5 transition-colors hover:border-[var(--gold)]/50">
      <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
        <Trophy size={10} />
        Leaderboards
      </span>
      <p class="mt-1.5 text-xs font-semibold text-[var(--gold)] opacity-60 transition-opacity group-hover:opacity-100">View &rarr;</p>
    </a>

    <div class="col-span-1 flex flex-col gap-2 md:gap-3">
      <a href="/world/items" class="pixel-box group flex-1 rounded-lg bg-[var(--panel)] px-3 py-2 transition-colors hover:border-[var(--gold)]/50">
        <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
          <Gem size={10} />
          Items
        </span>
      </a>
      <a href="/world/bestiary" class="pixel-box group flex-1 rounded-lg bg-[var(--panel)] px-3 py-2 transition-colors hover:border-[var(--gold)]/50">
        <span class="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
          <Skull size={10} />
          Bestiary
        </span>
      </a>
    </div>

    <div class="col-span-3 md:col-span-6">
      <section class="pixel-box h-full rounded-xl bg-[var(--panel)] p-4 md:p-5">
        <h2 class="flex items-center gap-2 text-sm font-bold text-[var(--text)]">
          <Crown size={14} class="text-[var(--gold)]" />
          Top Total Level
        </h2>
        <div class="mt-3 space-y-1.5">
          {#if loadingTopLevels}
            {#each Array(5) as _, i}
              <div class="h-8 animate-pulse rounded bg-[var(--panel-soft)]"></div>
            {/each}
          {:else if (topLevels ?? []).length === 0}
            <p class="py-3 text-sm text-[var(--text-soft)]">No data yet.</p>
          {:else}
            {#each topLevels ?? [] as entry, index}
              <div class="flex items-center justify-between rounded-lg bg-[var(--panel-soft)] px-3 py-2">
                <div class="flex items-center gap-2.5">
                  <span class="w-4 font-mono text-xs {index === 0 ? 'text-[var(--gold)]' : 'text-[var(--muted)]'}">{index + 1}</span>
                  <a href="/world/player/{encodeURIComponent(entry.name)}" class="text-sm font-medium text-[var(--text)] hover:text-[var(--gold)]">{entry.name}</a>
                </div>
                <div class="flex items-baseline gap-1.5">
                  <span class="font-mono text-sm text-[var(--text)]">{entry.total_level.toLocaleString()}</span>
                  <span class="text-[10px] text-[var(--muted)]">Total Lv</span>
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </section>
    </div>

    <div class="col-span-3 md:col-span-6">
      <section class="pixel-box h-full rounded-xl bg-[var(--panel)] p-4 md:p-5">
        <h2 class="flex items-center gap-2 text-sm font-bold text-[var(--text)]">
          <Crosshair size={14} class="text-[var(--ember)]" />
          Top Monster Hunters
        </h2>
        <div class="mt-3 space-y-1.5">
          {#if loadingTopHunters}
            {#each Array(5) as _, i}
              <div class="h-8 animate-pulse rounded bg-[var(--panel-soft)]"></div>
            {/each}
          {:else if (topHunters ?? []).length === 0}
            <p class="py-3 text-sm text-[var(--text-soft)]">No data yet.</p>
          {:else}
            {#each topHunters ?? [] as entry, index}
              <div class="flex items-center justify-between rounded-lg bg-[var(--panel-soft)] px-3 py-2">
                <div class="flex items-center gap-2.5">
                  <span class="w-4 font-mono text-xs {index === 0 ? 'text-[var(--gold)]' : 'text-[var(--muted)]'}">{index + 1}</span>
                  <a href="/world/player/{encodeURIComponent(entry.name)}" class="text-sm font-medium text-[var(--text)] hover:text-[var(--gold)]">{entry.name}</a>
                </div>
                <div class="flex items-baseline gap-1.5">
                  <span class="font-mono text-sm text-[var(--text)]">{entry.monster_kills.toLocaleString()}</span>
                  <span class="text-[10px] text-[var(--muted)]">Kills</span>
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </section>
    </div>
  </div>
</div>
