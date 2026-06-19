<script lang="ts">
  import { browser } from '$app/environment';
  import { onMount } from 'svelte';
  import { api, type LeaderboardEntry, type Overview } from '$lib/api';
  import WorldMap from '$lib/components/WorldMap.svelte';
  import { DEMO_LEADERBOARD, FALLBACK_OVERVIEW, resolveOverview } from '$lib/world-fallback';
  import {
    getPoi,
    getRegion,
    getRegionCenter,
    mapPois,
    worldActivities,
    worldRegions,
    worldStats,
  } from '$lib/world-guide';

  const statIcons = {
    towns: '🏰',
    regions: '🧭',
    pois: '📍',
    dungeons: '💀',
  } as const;

  let overview: Overview | undefined = $state(FALLBACK_OVERVIEW);
  let topLevels: LeaderboardEntry[] | undefined = $state(DEMO_LEADERBOARD.slice(0, 4));
  let loadingOverview = $state(false);
  let loadingTopLevels = $state(false);
  let usingDemoData = $state(true);
  let selectedPoiId = $state<string | null>(null);
  let selectedRegionId = $state('verdant');
  let centerRequest = $state<{ x: number; y: number; zoom?: number; n: number } | null>(null);
  let displayTime = $state('—');

  let selectedPoi = $derived(selectedPoiId ? getPoi(selectedPoiId) : null);
  let selectedRegion = $derived(getRegion(selectedRegionId));
  let regionPois = $derived(mapPois.filter((p) => p.regionId === selectedRegionId));

  function focusMap(x: number, y: number, zoom = 1.75) {
    centerRequest = { x, y, zoom, n: Date.now() };
  }

  function selectRegion(regionId: string) {
    selectedRegionId = regionId;
    selectedPoiId = null;
    const center = getRegionCenter(regionId);
    if (center) focusMap(center.x, center.y, 1.6);
    document.getElementById('region-overview')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }

  function selectPoi(poiId: string) {
    const poi = getPoi(poiId);
    if (!poi) return;
    selectedPoiId = poiId;
    selectedRegionId = poi.regionId;
    focusMap(poi.x, poi.y, 2.1);
  }

  function serverTime() {
    return new Date().toLocaleTimeString(undefined, {
      hour: 'numeric',
      minute: '2-digit',
      timeZoneName: 'short',
    });
  }

  async function load() {
    if (!browser) return;
    try {
      const [o, levels] = await Promise.all([
        api.overview(),
        api.leaderboard('total_level', 4),
      ]);
      const resolved = resolveOverview(o);
      overview = resolved.overview;
      topLevels = levels.length > 0 ? levels : DEMO_LEADERBOARD.slice(0, 4);
      usingDemoData = resolved.usingDemo || levels.length === 0;
    } catch {
      overview = FALLBACK_OVERVIEW;
      topLevels = DEMO_LEADERBOARD.slice(0, 4);
      usingDemoData = true;
    }
  }

  onMount(() => {
    document.title = 'The World of Solstead — Solstead';
    displayTime = serverTime();
    const timeId = setInterval(() => {
      displayTime = serverTime();
    }, 30_000);
    load();
    const id = setInterval(load, 15_000);
    return () => {
      clearInterval(id);
      clearInterval(timeId);
    };
  });
</script>

<div class="world-explorer">
  <aside class="world-panel world-panel-left">
    <div class="panel-inner ornate-panel">
      <p class="panel-kicker">Explore</p>
      <h1 class="panel-title">The World of Solstead</h1>
      <p class="panel-lead">
        A vast, persistent world shaped by its players. Explore land and sea, discover towns, dungeons,
        and resources — and leave your mark.
      </p>

      <dl class="world-stat-list">
        <div>
          <dt><span class="stat-icon" aria-hidden="true">{statIcons.towns}</span> Towns &amp; hubs</dt>
          <dd>{worldStats.towns}</dd>
        </div>
        <div>
          <dt><span class="stat-icon" aria-hidden="true">{statIcons.regions}</span> Regions</dt>
          <dd>{worldStats.regions}</dd>
        </div>
        <div>
          <dt><span class="stat-icon" aria-hidden="true">{statIcons.pois}</span> Points of interest</dt>
          <dd>{worldStats.pois}</dd>
        </div>
        <div>
          <dt><span class="stat-icon" aria-hidden="true">{statIcons.dungeons}</span> Dungeons</dt>
          <dd>{worldStats.dungeons}</dd>
        </div>
      </dl>

      <div class="region-quick-pick">
        <p class="panel-kicker mt-4">Regions</p>
        <ul class="region-quick-list">
          {#each worldRegions as region}
            <li>
              <button
                type="button"
                class="region-quick-btn {selectedRegionId === region.id && !selectedPoiId
                  ? 'is-active'
                  : ''}"
                onclick={() => selectRegion(region.id)}
              >
                {region.name}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    </div>

    <div class="panel-inner ornate-panel mt-3 hidden lg:block">
      <p class="panel-kicker">Activities</p>
      <ul class="activity-mini-list">
        {#each worldActivities as act}
          <li>
            <span aria-hidden="true">{act.icon}</span>
            <div>
              <strong>{act.title}</strong>
              <span>{act.summary}</span>
            </div>
          </li>
        {/each}
      </ul>
    </div>
  </aside>

  <section class="world-map-column" id="world-map">
    <WorldMap
      bind:selectedId={selectedPoiId}
      bind:selectedRegionId
      bind:centerRequest
    />
  </section>

  <aside class="world-panel world-panel-right" id="region-overview">
    <div class="panel-inner ornate-panel">
      <p class="panel-kicker">Region overview</p>
      {#if selectedPoi}
        <div class="region-thumb region-thumb-poi" aria-hidden="true">
          <span>{selectedPoi.name.slice(0, 1)}</span>
        </div>
        <h2 class="region-name">{selectedPoi.name}</h2>
        <p class="region-tag">{getRegion(selectedPoi.regionId)?.name ?? 'Solstead'}</p>
        <p class="region-desc">{selectedPoi.blurb}</p>
      {:else if selectedRegion}
        <div class="region-thumb" aria-hidden="true">
          <span>{selectedRegion.name.slice(0, 1)}</span>
        </div>
        <h2 class="region-name">{selectedRegion.name}</h2>
        <p class="region-tag">{selectedRegion.tagline}</p>
        <p class="region-desc">{selectedRegion.description}</p>
      {/if}

      {#if selectedRegion}
        <ul class="region-activities">
          {#each selectedRegion.activities as act}
            <li>{act}</li>
          {/each}
        </ul>
      {/if}

      {#if regionPois.length > 0}
        <div class="region-poi-list">
          <p class="panel-kicker mt-4">In this region</p>
          {#each regionPois as poi}
            <button
              type="button"
              class="region-poi-btn {selectedPoiId === poi.id ? 'is-active' : ''}"
              onclick={() => selectPoi(poi.id)}
            >
              <span>{poi.name}</span>
              <span class="poi-arrow" aria-hidden="true">→</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="panel-inner ornate-panel mt-3">
      <p class="panel-kicker">✦ Top adventurers</p>
      <ul class="discovery-list">
        {#if loadingTopLevels}
          {#each Array(3) as _}
            <li class="discovery-skeleton"></li>
          {/each}
        {:else if (topLevels ?? []).length === 0}
          <li class="discovery-empty">No explorers yet — be the first.</li>
        {:else}
          {#each topLevels ?? [] as entry, index}
            <li>
              <span class="discovery-rank">{index + 1}</span>
              <div>
                <a href="/world/player/{encodeURIComponent(entry.name)}">{entry.name}</a>
                <span>Total level {entry.total_level.toLocaleString()}</span>
              </div>
            </li>
          {/each}
        {/if}
      </ul>
      <a href="/world/leaderboards" class="world-guide-link mt-3">
        View leaderboards →
      </a>
    </div>

    <div class="panel-inner ornate-panel mt-3">
      <p class="panel-kicker">World status</p>
      <dl class="status-list">
        <div>
          <dt><span class="stat-icon" aria-hidden="true">📡</span> Players online</dt>
          <dd>
            {#if loadingOverview}
              <span class="pulse-stat">—</span>
            {:else}
              <span class="online-dot"></span>
              {(overview?.online_players ?? 0).toLocaleString()}
            {/if}
          </dd>
        </div>
        <div>
          <dt><span class="stat-icon" aria-hidden="true">👥</span> Characters</dt>
          <dd>{loadingOverview ? '—' : (overview?.total_characters ?? 0).toLocaleString()}</dd>
        </div>
        <div>
          <dt><span class="stat-icon" aria-hidden="true">📜</span> Server time</dt>
          <dd>{displayTime}</dd>
        </div>
      </dl>
      {#if usingDemoData}
        <p class="demo-notice">Live stats unavailable — showing sample data.</p>
      {/if}
      <div class="quick-links">
        <a href="/world/players">👥 Live players</a>
        <a href="/world/items">🏆 Items</a>
        <a href="/world/bestiary">💀 Bestiary</a>
      </div>
    </div>
  </aside>
</div>

<style>
  .world-explorer {
    display: grid;
    gap: 14px;
    grid-template-columns: 1fr;
    position: relative;
    z-index: 1;
  }

  @media (min-width: 1024px) {
    .world-explorer {
      grid-template-columns: minmax(220px, 260px) minmax(0, 1fr) minmax(240px, 280px);
      align-items: start;
    }
  }

  .world-panel {
    display: flex;
    flex-direction: column;
  }

  .panel-inner {
    padding: 16px;
  }

  .ornate-panel {
    position: relative;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background:
      linear-gradient(180deg, rgba(22, 16, 12, 0.95), rgba(14, 10, 8, 0.98)),
      var(--panel);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .ornate-panel::before,
  .ornate-panel::after {
    content: '';
    position: absolute;
    width: 10px;
    height: 10px;
    border-color: color-mix(in oklab, var(--gold) 50%, transparent);
    border-style: solid;
    pointer-events: none;
  }

  .ornate-panel::before {
    top: 5px;
    left: 5px;
    border-width: 1px 0 0 1px;
  }

  .ornate-panel::after {
    bottom: 5px;
    right: 5px;
    border-width: 0 1px 1px 0;
  }

  .panel-kicker {
    margin: 0;
    font-family: var(--font-display);
    font-size: 8px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .panel-title {
    margin: 8px 0 0;
    font-size: 14px;
    line-height: 1.35;
    color: var(--gold-light);
  }

  .panel-lead {
    margin: 10px 0 0;
    font-size: 12px;
    line-height: 1.55;
    color: var(--text-soft);
  }

  .world-stat-list {
    margin: 16px 0 0;
    display: grid;
    gap: 10px;
  }

  .world-stat-list div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border-bottom: 1px solid rgba(90, 64, 48, 0.35);
    padding-bottom: 8px;
  }

  .world-stat-list dt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .stat-icon {
    font-size: 11px;
    line-height: 1;
  }

  .poi-arrow {
    font-size: 10px;
    color: var(--gold);
  }

  .world-stat-list dd {
    margin: 0;
    font-family: var(--font-display);
    font-size: 14px;
    color: var(--text);
  }

  .region-quick-pick {
    margin-top: 4px;
  }

  .region-quick-list {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 4px;
  }

  .region-quick-btn {
    display: block;
    width: 100%;
    border: 1px solid transparent;
    background: rgba(0, 0, 0, 0.12);
    padding: 6px 8px;
    font-size: 11px;
    text-align: left;
    color: var(--text-soft);
    cursor: pointer;
    transition:
      border-color 0.15s,
      color 0.15s,
      background 0.15s;
  }

  .region-quick-btn:hover,
  .region-quick-btn.is-active {
    border-color: color-mix(in oklab, var(--gold) 40%, transparent);
    background: rgba(212, 168, 68, 0.08);
    color: var(--gold-light);
  }

  .world-guide-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 14px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--gold);
    text-decoration: none;
    transition: color 0.15s;
  }

  .world-guide-link:hover {
    color: var(--gold-light);
  }

  .activity-mini-list {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 8px;
  }

  .activity-mini-list li {
    display: flex;
    gap: 8px;
    font-size: 11px;
    color: var(--text-soft);
  }

  .activity-mini-list strong {
    display: block;
    font-size: 11px;
    color: var(--text);
  }

  .activity-mini-list span {
    display: block;
    margin-top: 1px;
    font-size: 10px;
    line-height: 1.35;
  }

  .world-map-column {
    min-width: 0;
  }

  .region-thumb {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 72px;
    margin-top: 10px;
    border: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
    background:
      radial-gradient(ellipse at 50% 30%, rgba(212, 168, 68, 0.15), transparent 60%),
      linear-gradient(180deg, rgba(42, 30, 20, 0.8), rgba(18, 12, 8, 0.95));
    font-family: var(--font-display);
    font-size: 28px;
    color: var(--gold);
  }

  .region-thumb-poi span {
    font-size: 22px;
  }

  .region-name {
    margin: 10px 0 0;
    font-size: 13px;
    color: var(--gold-light);
  }

  .region-tag {
    margin: 4px 0 0;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .region-desc {
    margin: 10px 0 0;
    font-size: 12px;
    line-height: 1.55;
    color: var(--text-soft);
  }

  .region-activities {
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .region-activities li {
    border: 1px solid rgba(90, 64, 48, 0.5);
    padding: 3px 8px;
    font-size: 10px;
    color: var(--text-soft);
  }

  .region-poi-list {
    display: grid;
    gap: 4px;
  }

  .region-poi-btn {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    border: 1px solid transparent;
    background: rgba(0, 0, 0, 0.15);
    padding: 6px 8px;
    font-size: 11px;
    color: var(--text-soft);
    cursor: pointer;
    text-align: left;
    transition:
      border-color 0.15s,
      color 0.15s;
  }

  .region-poi-btn:hover,
  .region-poi-btn.is-active {
    border-color: color-mix(in oklab, var(--gold) 40%, transparent);
    color: var(--gold-light);
  }

  .discovery-list {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 8px;
  }

  .discovery-list li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  .discovery-rank {
    font-family: var(--font-display);
    font-size: 10px;
    color: var(--gold);
    min-width: 14px;
  }

  .discovery-list a {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    text-decoration: none;
  }

  .discovery-list a:hover {
    color: var(--gold);
  }

  .discovery-list span {
    display: block;
    margin-top: 1px;
    font-size: 10px;
    color: var(--muted);
  }

  .discovery-skeleton {
    height: 32px;
    border-radius: 4px;
    background: var(--panel-soft);
    animation: pulse 1.5s ease-in-out infinite;
  }

  .discovery-empty {
    font-size: 11px;
    color: var(--muted);
  }

  .status-list {
    margin: 10px 0 0;
    display: grid;
    gap: 10px;
  }

  .status-list div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .status-list dt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .status-list dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-display);
    font-size: 13px;
    color: var(--text);
  }

  .online-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--moss-light);
    box-shadow: 0 0 6px var(--moss-light);
  }

  .quick-links {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid rgba(90, 64, 48, 0.35);
  }

  .quick-links a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-soft);
    text-decoration: none;
  }

  .quick-links a:hover {
    color: var(--gold);
  }

  .demo-notice {
    margin: 10px 0 0;
    font-size: 10px;
    font-style: italic;
    color: var(--muted);
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.5;
    }
    50% {
      opacity: 1;
    }
  }
</style>
