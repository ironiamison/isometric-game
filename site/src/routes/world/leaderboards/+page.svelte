<script lang="ts">
  import { browser } from '$app/environment';
  import { onMount } from 'svelte';
  import { api, type LeaderboardEntry } from '$lib/api';
  import {
    LEADERBOARD_CATEGORIES,
    MOCK_TOP_CLANS,
    avatarHue,
    clanForPlayer,
    displayLevel,
    formatScore,
    seasonCountdown,
    type LeaderboardCategory,
  } from '$lib/leaderboard-ui';
  import { DEMO_LEADERBOARD } from '$lib/world-fallback';
  import { ChevronLeft, ChevronRight, Clock, Hourglass, Shield, Swords, Trophy } from '@lucide/svelte';

  let activeCategoryId = $state('overall');
  let currentPage = $state(1);
  let data: LeaderboardEntry[] | undefined = $state();
  let isLoading = $state(true);
  let usingDemoData = $state(false);
  let countdown = $state('—');
  const pageSize = 10;

  let category = $derived(
    LEADERBOARD_CATEGORIES.find((c) => c.id === activeCategoryId) ?? LEADERBOARD_CATEGORIES[0],
  );

  let ranked = $derived((data ?? []).map((entry, index) => ({ rank: index + 1, entry })));
  let totalPages = $derived(Math.max(1, Math.ceil(ranked.length / pageSize)));
  let pageRows = $derived(ranked.slice((currentPage - 1) * pageSize, currentPage * pageSize));

  let pageNumbers = $derived.by(() => {
    const nums: (number | 'ellipsis')[] = [];
    if (totalPages <= 7) {
      for (let i = 1; i <= totalPages; i++) nums.push(i);
      return nums;
    }
    nums.push(1, 2, 3, 4, 5, 'ellipsis', totalPages);
    return nums;
  });

  async function load(cat: LeaderboardCategory) {
    isLoading = true;
    try {
      const rows = await api.leaderboard(cat.sort, 100);
      data = rows.length > 0 ? rows : DEMO_LEADERBOARD;
      usingDemoData = rows.length === 0;
    } catch {
      data = cat.id === 'clans' ? [] : DEMO_LEADERBOARD;
      usingDemoData = cat.id !== 'clans';
    } finally {
      isLoading = false;
    }
  }

  function selectCategory(id: string) {
    activeCategoryId = id;
    currentPage = 1;
  }

  $effect(() => {
    if (!browser) return;
    load(category);
  });

  let isClansView = $derived(activeCategoryId === 'clans');

  function rankClass(rank: number) {
    if (rank === 1) return 'rank-gold';
    if (rank === 2) return 'rank-silver';
    if (rank === 3) return 'rank-bronze';
    return '';
  }

  onMount(() => {
    document.title = 'Leaderboards — Solstead';
    countdown = seasonCountdown();
    const id = setInterval(() => {
      countdown = seasonCountdown();
    }, 60_000);
    return () => clearInterval(id);
  });
</script>

<svelte:head>
  <title>Leaderboards — Solstead</title>
  <meta name="description" content="Solstead player leaderboards — overall rankings, combat, skills, and more." />
  <link rel="canonical" href="https://solstead.xyz/world/leaderboards/" />
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Cinzel:wght@500;600;700&display=swap" />
</svelte:head>

<div class="lb-grid">
  <aside class="lb-sidebar">
    <div class="lb-panel">
      <h2 class="lb-panel-title">Categories</h2>
      <ul class="lb-categories">
        {#each LEADERBOARD_CATEGORIES as cat}
          <li>
            <button
              type="button"
              class="lb-category {activeCategoryId === cat.id ? 'is-active' : ''}"
              onclick={() => selectCategory(cat.id)}
            >
              <span class="lb-cat-icon" aria-hidden="true">{cat.icon}</span>
              {cat.label}
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div class="lb-panel lb-season">
      <div class="lb-season-head">
        <Hourglass size={14} class="text-[var(--gold)]" />
        <span>Season 1</span>
      </div>
      <p class="lb-season-dates">May 1, 2024 – Aug 1, 2024</p>
      <p class="lb-season-countdown">
        <Clock size={11} />
        Season ends in: <strong>{countdown}</strong>
      </p>
    </div>
  </aside>

  <section class="lb-main">
    <header class="lb-main-header">
      <h1>{category.title.toUpperCase()}</h1>
      <p>{category.subtitle}</p>
      {#if usingDemoData && !isClansView}
        <p class="lb-demo-notice">Live rankings unavailable — showing sample adventurers.</p>
      {/if}
    </header>

    <div class="lb-panel lb-table-panel">
      {#if isClansView}
        <table class="lb-table">
          <thead>
            <tr>
              <th>Rank</th>
              <th>Clan</th>
              <th>Clan XP</th>
            </tr>
          </thead>
          <tbody>
            {#each MOCK_TOP_CLANS as clan}
              <tr>
                <td>
                  <span class="lb-rank {rankClass(clan.rank)}">
                    {#if clan.rank <= 3}
                      <span class="lb-wreath" aria-hidden="true">❧</span>
                    {/if}
                    {clan.rank}
                  </span>
                </td>
                <td>
                  <div class="lb-player">
                    <span class="lb-clan-icon-lg" aria-hidden="true">{clan.icon}</span>
                    <span class="lb-player-name">{clan.name}</span>
                  </div>
                </td>
                <td class="lb-num lb-xp">{clan.xp.toLocaleString()}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
      <table class="lb-table">
        <thead>
          <tr>
            <th>Rank</th>
            <th>Player</th>
            <th>Level</th>
            <th>{category.valueLabel}</th>
          </tr>
        </thead>
        <tbody>
          {#if isLoading}
            {#each Array(pageSize) as _}
              <tr>
                <td colspan="4"><div class="lb-skeleton"></div></td>
              </tr>
            {/each}
          {:else if pageRows.length === 0}
            <tr>
              <td colspan="4" class="lb-empty">No rankings yet — be the first to chart.</td>
            </tr>
          {:else}
            {#each pageRows as { rank, entry }}
              {@const clan = clanForPlayer(entry.name)}
              <tr>
                <td>
                  <span class="lb-rank {rankClass(rank)}">
                    {#if rank <= 3}
                      <span class="lb-wreath" aria-hidden="true">❧</span>
                    {/if}
                    {rank}
                  </span>
                </td>
                <td>
                  <div class="lb-player">
                    <span
                      class="lb-avatar"
                      style="--av-hue: {avatarHue(entry.name)}"
                      aria-hidden="true"
                    ></span>
                    <div>
                      <a href="/world/player/{encodeURIComponent(entry.name)}" class="lb-player-name">
                        {entry.name}
                      </a>
                      <span class="lb-clan">
                        <span aria-hidden="true">{clan.icon}</span>
                        {clan.name}
                      </span>
                    </div>
                  </div>
                </td>
                <td class="lb-num">{displayLevel(category, entry)}</td>
                <td class="lb-num lb-xp">{formatScore(category, entry)}</td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>

      <div class="lb-pagination">
        <button
          type="button"
          class="lb-page-btn"
          disabled={currentPage <= 1}
          aria-label="Previous page"
          onclick={() => (currentPage = Math.max(1, currentPage - 1))}
        >
          <ChevronLeft size={14} />
        </button>
        {#each pageNumbers as n}
          {#if n === 'ellipsis'}
            <span class="lb-page-ellipsis">…</span>
          {:else}
            <button
              type="button"
              class="lb-page-num {currentPage === n ? 'is-active' : ''}"
              onclick={() => (currentPage = n)}
            >
              {n}
            </button>
          {/if}
        {/each}
        <button
          type="button"
          class="lb-page-btn"
          disabled={currentPage >= totalPages}
          aria-label="Next page"
          onclick={() => (currentPage = Math.min(totalPages, currentPage + 1))}
        >
          <ChevronRight size={14} />
        </button>
      </div>
      {/if}
    </div>
  </section>

  <aside class="lb-rail">
    <div class="lb-panel">
      <h2 class="lb-panel-title">Your Rank</h2>
      <div class="lb-your-rank">
        <span class="lb-avatar lb-avatar-you" aria-hidden="true"></span>
        <div>
          <p class="lb-you-label">You</p>
          <p class="lb-you-meta">Play to appear on the board</p>
        </div>
        <div class="lb-you-stats">
          <span class="lb-you-rank-num">—</span>
        </div>
      </div>
      <p class="lb-you-xp-label">Total XP</p>
      <p class="lb-you-xp">—</p>
      <a href="/play/index.html" class="lb-cta-outline">Enter Solstead</a>
    </div>

    <div class="lb-panel">
      <h2 class="lb-panel-title">About Leaderboards</h2>
      <ul class="lb-about">
        <li><Trophy size={12} /> Rankings refresh from live game data.</li>
        <li><Swords size={12} /> Compete in combat, skills, and exploration.</li>
        <li><Shield size={12} /> Season rewards planned as Solstead grows.</li>
      </ul>
      <button type="button" class="lb-cta-outline" disabled>View Rewards</button>
    </div>

    <div class="lb-panel">
      <h2 class="lb-panel-title">Top Clans</h2>
      <ol class="lb-clans">
        {#each MOCK_TOP_CLANS as clan}
          <li>
            <span class="lb-clan-rank">{clan.rank}</span>
            <span class="lb-clan-icon" aria-hidden="true">{clan.icon}</span>
            <span class="lb-clan-name">{clan.name}</span>
            <span class="lb-clan-xp">{clan.xp.toLocaleString()}</span>
          </li>
        {/each}
      </ol>
      <button type="button" class="lb-cta-outline" onclick={() => selectCategory('clans')}>
        View Clan Leaderboard
      </button>
    </div>
  </aside>
</div>

<style>
  .lb-grid {
    display: grid;
    gap: 14px;
    grid-template-columns: 1fr;
    max-width: 1320px;
    margin: 0 auto;
  }

  @media (min-width: 1024px) {
    .lb-grid {
      grid-template-columns: 200px minmax(0, 1fr) 250px;
      align-items: start;
    }
  }

  .lb-panel {
    position: relative;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background: rgba(12, 8, 6, 0.82);
    padding: 14px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .lb-panel::before,
  .lb-panel::after {
    content: '';
    position: absolute;
    width: 10px;
    height: 10px;
    border-color: color-mix(in oklab, var(--gold) 55%, transparent);
    border-style: solid;
    pointer-events: none;
  }

  .lb-panel::before {
    top: 4px;
    left: 4px;
    border-width: 1px 0 0 1px;
  }

  .lb-panel::after {
    bottom: 4px;
    right: 4px;
    border-width: 0 1px 1px 0;
  }

  .lb-panel-title {
    margin: 0 0 10px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .lb-categories {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 4px;
  }

  .lb-category {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    border: 1px solid transparent;
    background: transparent;
    padding: 8px 10px;
    font-size: 12px;
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }

  .lb-category:hover {
    color: var(--text);
    background: rgba(212, 168, 68, 0.06);
  }

  .lb-category.is-active {
    border-color: color-mix(in oklab, var(--gold) 50%, transparent);
    background: rgba(212, 168, 68, 0.1);
    color: var(--gold-light);
  }

  .lb-cat-icon {
    width: 18px;
    text-align: center;
    font-size: 13px;
  }

  .lb-season {
    margin-top: 10px;
  }

  .lb-season-head {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-display);
    font-size: 11px;
    letter-spacing: 0.1em;
    color: var(--gold-light);
  }

  .lb-season-dates {
    margin: 6px 0 0;
    font-size: 10px;
    color: var(--muted);
  }

  .lb-season-countdown {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    font-size: 10px;
    color: var(--text-soft);
  }

  .lb-season-countdown strong {
    color: var(--gold);
  }

  .lb-main-header h1 {
    margin: 0;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: clamp(1.25rem, 2.5vw, 1.75rem);
    letter-spacing: 0.12em;
    color: var(--gold-light);
    text-shadow: 0 0 24px rgba(212, 168, 68, 0.25);
  }

  .lb-main-header p {
    margin: 8px 0 14px;
    font-size: 12px;
    color: var(--text-soft);
  }

  .lb-demo-notice {
    margin: -6px 0 14px;
    font-size: 11px;
    font-style: italic;
    color: var(--muted);
  }

  .lb-clan-icon-lg {
    font-size: 18px;
    line-height: 1;
  }

  .lb-table-panel {
    padding: 0;
    overflow: hidden;
  }

  .lb-table {
    width: 100%;
    border-collapse: collapse;
  }

  .lb-table th {
    padding: 12px 14px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    text-align: left;
    color: var(--muted);
    border-bottom: 1px solid rgba(90, 64, 48, 0.45);
    background: rgba(0, 0, 0, 0.2);
  }

  .lb-table th:last-child,
  .lb-table td.lb-num {
    text-align: right;
  }

  .lb-table td {
    padding: 11px 14px;
    border-bottom: 1px solid rgba(90, 64, 48, 0.25);
    vertical-align: middle;
  }

  .lb-table tbody tr:hover {
    background: rgba(212, 168, 68, 0.04);
  }

  .lb-rank {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    font-family: var(--font-display);
    font-size: 13px;
    color: var(--text-soft);
    position: relative;
  }

  .lb-rank.rank-gold {
    color: var(--gold-light);
  }

  .lb-rank.rank-silver {
    color: #c0c0c0;
  }

  .lb-rank.rank-bronze {
    color: #cd7f32;
  }

  .lb-wreath {
    position: absolute;
    left: -6px;
    right: -6px;
    font-size: 22px;
    opacity: 0.55;
    pointer-events: none;
  }

  .lb-player {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .lb-avatar {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border: 1px solid rgba(212, 168, 68, 0.35);
    background:
      linear-gradient(180deg, hsl(var(--av-hue) 45% 42%), hsl(var(--av-hue) 35% 28%)),
      repeating-linear-gradient(0deg, rgba(0, 0, 0, 0.08) 0 2px, transparent 2px 4px);
    image-rendering: pixelated;
    box-shadow: inset 0 0 0 2px rgba(0, 0, 0, 0.25);
  }

  .lb-avatar-you {
    --av-hue: 38;
    opacity: 0.5;
  }

  .lb-player-name {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: var(--gold-light);
    text-decoration: none;
  }

  .lb-player-name:hover {
    color: var(--gold);
  }

  .lb-clan {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
    font-size: 10px;
    color: var(--muted);
  }

  .lb-num {
    font-family: var(--font-display);
    font-size: 12px;
    color: var(--text);
  }

  .lb-xp {
    color: var(--text-soft);
  }

  .lb-empty {
    padding: 32px !important;
    text-align: center;
    color: var(--muted);
    font-size: 13px;
  }

  .lb-skeleton {
    height: 36px;
    background: rgba(255, 255, 255, 0.04);
    animation: pulse 1.5s ease-in-out infinite;
  }

  .lb-pagination {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 12px;
    border-top: 1px solid rgba(90, 64, 48, 0.35);
  }

  .lb-page-btn,
  .lb-page-num {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 32px;
    height: 32px;
    border: 1px solid color-mix(in oklab, var(--gold) 30%, transparent);
    background: rgba(0, 0, 0, 0.25);
    color: var(--text-soft);
    cursor: pointer;
    font-family: var(--font-display);
    font-size: 11px;
    transition:
      border-color 0.15s,
      background 0.15s,
      color 0.15s;
  }

  .lb-page-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .lb-page-num.is-active {
    background: color-mix(in oklab, var(--gold) 25%, transparent);
    border-color: var(--gold);
    color: var(--gold-light);
  }

  .lb-page-ellipsis {
    padding: 0 4px;
    color: var(--muted);
    font-size: 12px;
  }

  .lb-rail {
    display: grid;
    gap: 10px;
  }

  .lb-your-rank {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
  }

  .lb-you-label {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .lb-you-meta {
    margin: 2px 0 0;
    font-size: 10px;
    color: var(--muted);
  }

  .lb-you-rank-num {
    font-family: var(--font-display);
    font-size: 16px;
    color: var(--gold);
  }

  .lb-you-xp-label {
    margin: 12px 0 2px;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .lb-you-xp {
    margin: 0 0 12px;
    font-family: var(--font-display);
    font-size: 18px;
    color: var(--text-soft);
  }

  .lb-about {
    margin: 0 0 12px;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 8px;
  }

  .lb-about li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-soft);
  }

  .lb-about :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--gold);
  }

  .lb-cta-outline {
    display: block;
    width: 100%;
    border: 1px solid color-mix(in oklab, var(--gold) 45%, transparent);
    background: transparent;
    padding: 9px 12px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    text-align: center;
    text-decoration: none;
    color: var(--gold);
    cursor: pointer;
    transition: background 0.15s;
  }

  .lb-cta-outline:hover:not(:disabled) {
    background: rgba(212, 168, 68, 0.1);
  }

  .lb-cta-outline:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .lb-clans {
    margin: 0 0 12px;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 6px;
  }

  .lb-clans li {
    display: grid;
    grid-template-columns: 18px 20px 1fr auto;
    gap: 6px;
    align-items: center;
    font-size: 11px;
  }

  .lb-clan-rank {
    font-family: var(--font-display);
    color: var(--muted);
  }

  .lb-clan-name {
    color: var(--text-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lb-clan-xp {
    font-family: var(--font-display);
    font-size: 10px;
    color: var(--muted);
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.45;
    }
    50% {
      opacity: 1;
    }
  }

  @media (max-width: 1023px) {
    .lb-sidebar {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 10px;
    }

    .lb-season {
      margin-top: 0;
    }

    .lb-rail {
      grid-template-columns: 1fr 1fr;
    }
  }

  @media (max-width: 640px) {
    .lb-sidebar,
    .lb-rail {
      grid-template-columns: 1fr;
    }

    .lb-table th:nth-child(3),
    .lb-table td:nth-child(3) {
      display: none;
    }
  }
</style>
