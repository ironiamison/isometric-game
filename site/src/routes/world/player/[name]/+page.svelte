<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/stores';
  import { api, type LeaderboardEntry, type PlayerProfileRanks } from '$lib/api';
  import { getDemoPlayerProfile } from '$lib/world-fallback';
  import { formatPlayedTime, percentile } from '$lib/format';
  import { ArrowLeft, Check, Copy, UserRound } from '@lucide/svelte';

  type ProfileStat = {
    label: string;
    value: (player: LeaderboardEntry) => string;
    rank: (ranks: PlayerProfileRanks) => number;
  };

  const PROFILE_STATS: ProfileStat[] = [
    { label: 'Total Level', value: (p) => p.total_level.toLocaleString(), rank: (r) => r.total_level },
    { label: 'Combat Level', value: (p) => p.combat_level.toLocaleString(), rank: (r) => r.combat_level },
    { label: 'Attack', value: (p) => p.attack_level.toLocaleString(), rank: (r) => r.attack_level },
    { label: 'Strength', value: (p) => p.strength_level.toLocaleString(), rank: (r) => r.strength_level },
    { label: 'Defence', value: (p) => p.defence_level.toLocaleString(), rank: (r) => r.defence_level },
    { label: 'Ranged', value: (p) => p.ranged_level.toLocaleString(), rank: (r) => r.ranged_level },
    { label: 'Hitpoints', value: (p) => p.hitpoints_level.toLocaleString(), rank: (r) => r.hitpoints_level },
    { label: 'Fishing', value: (p) => p.fishing_level.toLocaleString(), rank: (r) => r.fishing_level },
    { label: 'Farming', value: (p) => p.farming_level.toLocaleString(), rank: (r) => r.farming_level },
    { label: 'Woodcutting', value: (p) => p.woodcutting_level.toLocaleString(), rank: (r) => r.woodcutting_level },
    { label: 'Mining', value: (p) => p.mining_level.toLocaleString(), rank: (r) => r.mining_level },
    { label: 'Smithing', value: (p) => p.smithing_level.toLocaleString(), rank: (r) => r.smithing_level },
    { label: 'Alchemy', value: (p) => p.alchemy_level.toLocaleString(), rank: (r) => r.alchemy_level },
    { label: 'Prayer', value: (p) => p.prayer_level.toLocaleString(), rank: (r) => r.prayer_level },
    { label: 'Magic', value: (p) => p.magic_level.toLocaleString(), rank: (r) => r.magic_level },
    { label: 'Slayer', value: (p) => p.slayer_level.toLocaleString(), rank: (r) => r.slayer_level },
    { label: 'Survivalist', value: (p) => p.survivalist_level.toLocaleString(), rank: (r) => r.survivalist_level },
    { label: 'Monster Kills', value: (p) => p.monster_kills.toLocaleString(), rank: (r) => r.monster_kills },
    { label: 'Played Time', value: (p) => formatPlayedTime(p.played_time), rank: (r) => r.played_time },
  ];

  let playerName = $derived($page.params.name ?? '');
  let data = $state<Awaited<ReturnType<typeof api.playerProfile>> | undefined>();
  let isLoading = $state(true);
  let isError = $state(false);
  let usingDemoData = $state(false);
  let copied = $state(false);

  let sharePath = $derived(`/world/player/${encodeURIComponent(data?.player.name ?? playerName)}`);

  async function load(name: string) {
    if (!browser || !name) return;
    isLoading = true;
    isError = false;
    usingDemoData = false;
    try {
      data = await api.playerProfile(name);
    } catch {
      const demo = getDemoPlayerProfile(name);
      if (demo) {
        data = demo;
        usingDemoData = true;
      } else {
        isError = true;
        data = undefined;
      }
    } finally {
      isLoading = false;
    }
  }

  async function copyUrl() {
    const url = `${window.location.origin}${sharePath}`;
    try {
      await navigator.clipboard.writeText(url);
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      copied = false;
    }
  }

  $effect(() => {
    document.title = playerName
      ? `${playerName} — Solstead Player Profile`
      : 'Player Profile — Solstead World Statistics';
    load(playerName);
  });
</script>

{#if !playerName}
  <div class="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
    <p class="text-[var(--text-soft)]">Missing player name.</p>
  </div>
{:else if isLoading}
  <div class="space-y-4">
    <div class="h-36 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]"></div>
    <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
      {#each Array(8) as _, i}
        <div class="h-28 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]"></div>
      {/each}
    </div>
  </div>
{:else if isError || !data}
  <div class="space-y-4 rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
    <h1 class="text-2xl text-[var(--text)]">Player not found</h1>
    <p class="text-[var(--text-soft)]">No profile data exists for "{playerName}".</p>
    <a href="/world/leaderboards" class="pixel-btn inline-flex rounded-md bg-[var(--panel-soft)] px-3 py-2 text-sm text-[var(--text)]">Back to leaderboards</a>
  </div>
{:else}
  {@const { player, ranks, total_characters } = data}
  <div class="space-y-5">
    <section class="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_25%_10%,rgba(212,168,68,0.22),transparent_50%),radial-gradient(circle_at_90%_0%,rgba(90,114,71,0.16),transparent_48%),var(--panel)] p-6 md:p-7">
      <p class="flex items-center gap-2 text-xs tracking-[0.22em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">
        <UserRound size={14} class="text-[var(--gold)]" />
        Player Showcase
      </p>
      <h1 class="mt-2 text-4xl text-[var(--text)]">{player.name}</h1>
      <div class="mt-3 flex flex-wrap gap-2 text-sm">
        <span class="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">#{ranks.total_level} Total Level</span>
        <span class="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">#{ranks.monster_kills} Monster Kills</span>
        <span class="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">{percentile(ranks.total_level, total_characters)}</span>
      </div>
      <div class="mt-5 flex flex-wrap gap-3">
        <button onclick={copyUrl} class="pixel-btn inline-flex items-center gap-1.5 rounded-md bg-[var(--gold)] px-4 py-2 text-sm font-bold text-[#1a1210] hover:bg-[var(--gold-light)]">
          {#if copied}
            <Check size={14} /> Link copied
          {:else}
            <Copy size={14} /> Copy profile URL
          {/if}
        </button>
        <a href="/world/leaderboards" class="pixel-btn inline-flex items-center gap-1.5 rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm font-bold text-[var(--text-soft)] hover:text-[var(--text)]">
          <ArrowLeft size={14} />
          Leaderboards
        </a>
      </div>
      <p class="mt-3 text-xs text-[var(--muted)]">{sharePath}</p>
      {#if usingDemoData}
        <p class="mt-2 text-xs text-[var(--muted)]">Sample profile — live data unavailable for this adventurer.</p>
      {/if}
    </section>

    <section class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
      {#each PROFILE_STATS as stat}
        <article class="pixel-box rounded-xl bg-[var(--panel)] p-4">
          <p class="text-[11px] tracking-[0.14em] text-[var(--muted)] uppercase" style="font-family: var(--font-display)">{stat.label}</p>
          <p class="mt-2 text-2xl font-bold text-[var(--text)]">{stat.value(player)}</p>
          <p class="mt-2 text-xs text-[var(--text-soft)]">
            Global rank <span class="font-mono text-[var(--gold)]">#{stat.rank(ranks)}</span>
          </p>
        </article>
      {/each}
    </section>
  </div>
{/if}
