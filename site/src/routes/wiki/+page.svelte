<script lang="ts">
  import { goto } from '$app/navigation';
  import {
    EXPLORE_TILES,
    WIKI_STATS,
    formatDate,
    recentArticles,
    searchArticles,
  } from '$lib/wiki';

  let hubQuery = $state('');
  let recent = recentArticles().slice(0, 3);

  function onHubSearch(e: Event) {
    e.preventDefault();
    const q = hubQuery.trim();
    if (!q) return;
    const hits = searchArticles(q, 1);
    if (hits.length > 0) goto(`/wiki/${hits[0].slug}`);
    else goto('/wiki/articles');
  }
</script>

<svelte:head>
  <title>Solstead Wiki — Guides, Items, Combat & World</title>
  <meta
    name="description"
    content="Official Solstead wiki — {WIKI_STATS.items} items, {WIKI_STATS.quests} quests, combat guides, regions, dungeons, and more."
  />
  <link rel="canonical" href="https://solstead.xyz/wiki/" />
</svelte:head>

<section class="wiki-hero ornate-panel">
  <div class="wiki-hero-art" aria-hidden="true"></div>
  <div class="wiki-hero-content">
    <h1>Welcome to Solstead Wiki</h1>
    <p>
      Your complete guide to the world of Solstead — {WIKI_STATS.items} items, {WIKI_STATS.recipes} recipes,
      {WIKI_STATS.quests} quests, and every region, monster, and dungeon in the game.
    </p>
    <form class="wiki-hero-search" onsubmit={onHubSearch}>
      <input
        type="search"
        placeholder="Search for articles..."
        bind:value={hubQuery}
        aria-label="Search wiki articles"
      />
      <button type="submit">Search</button>
    </form>
  </div>
</section>

<section class="wiki-section">
  <h2 class="wiki-section-title">Explore Solstead</h2>
  <div class="wiki-tiles">
    {#each EXPLORE_TILES as tile}
      <a href="/wiki/{tile.slug}" class="wiki-tile ornate-panel">
        <img src={tile.img} alt="" class="wiki-tile-img" />
        <h3>{tile.label}</h3>
        <p>{tile.desc}</p>
      </a>
    {/each}
  </div>
</section>

<section class="wiki-section">
  <h2 class="wiki-section-title">Recently Added Articles</h2>
  <ul class="wiki-recent-list">
    {#each recent as article}
      <li>
        <a href="/wiki/{article.slug}" class="wiki-recent-item ornate-panel">
          <div class="wiki-recent-thumb">
            {#if article.thumbnail}
              <img src={article.thumbnail} alt="" />
            {:else}
              <span class="wiki-recent-fallback" aria-hidden="true">{article.icon ?? '📄'}</span>
            {/if}
          </div>
          <div class="wiki-recent-body">
            <h3>{article.title}</h3>
            <p>{article.summary}</p>
          </div>
          <time datetime={article.updatedAt}>{formatDate(article.updatedAt)}</time>
        </a>
      </li>
    {/each}
  </ul>
  <a href="/wiki/articles" class="wiki-browse-btn">
    <span aria-hidden="true">📖</span>
    Browse All Articles
  </a>
</section>

<style>
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
    z-index: 2;
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

  .wiki-hero {
    overflow: hidden;
    margin-bottom: 20px;
  }

  .wiki-hero-art {
    height: 180px;
    background: url('/wiki/wiki-hero.png') center / cover no-repeat;
    mask-image: linear-gradient(180deg, black 60%, transparent);
  }

  @media (min-width: 640px) {
    .wiki-hero-art {
      height: 220px;
    }
  }

  .wiki-hero-content {
    padding: 0 20px 24px;
    margin-top: -48px;
    position: relative;
    z-index: 1;
    text-align: center;
  }

  .wiki-hero-content h1 {
    margin: 0;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: clamp(22px, 4vw, 32px);
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--gold-light);
    text-shadow: 0 2px 12px rgba(0, 0, 0, 0.6);
  }

  .wiki-hero-content p {
    max-width: 560px;
    margin: 10px auto 0;
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-soft);
  }

  .wiki-hero-search {
    display: flex;
    max-width: 420px;
    margin: 18px auto 0;
    border: 1px solid color-mix(in oklab, var(--gold) 40%, transparent);
    background: rgba(8, 6, 4, 0.9);
  }

  .wiki-hero-search input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 11px 14px;
    font-size: 13px;
    color: var(--text);
    outline: none;
  }

  .wiki-hero-search button {
    border: none;
    border-left: 1px solid color-mix(in oklab, var(--gold) 30%, transparent);
    background: color-mix(in oklab, var(--gold) 12%, transparent);
    padding: 0 16px;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--gold);
    cursor: pointer;
  }

  .wiki-section {
    margin-bottom: 24px;
  }

  .wiki-section-title {
    margin: 0 0 12px;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .wiki-tiles {
    display: grid;
    gap: 10px;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  @media (min-width: 768px) {
    .wiki-tiles {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  .wiki-tile {
    display: block;
    padding: 12px;
    text-decoration: none;
    transition:
      border-color 0.15s,
      transform 0.15s;
  }

  .wiki-tile:hover {
    border-color: var(--gold);
    transform: translateY(-2px);
  }

  .wiki-tile-img {
    width: 64px;
    height: 64px;
    object-fit: cover;
    image-rendering: pixelated;
    border: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
  }

  .wiki-tile h3 {
    margin: 10px 0 4px;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 11px;
    letter-spacing: 0.08em;
    color: var(--gold-light);
  }

  .wiki-tile p {
    margin: 0;
    font-size: 11px;
    line-height: 1.45;
    color: var(--muted);
  }

  .wiki-recent-list {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: 10px;
  }

  .wiki-recent-item {
    display: grid;
    grid-template-columns: 80px 1fr auto;
    gap: 12px;
    align-items: center;
    padding: 10px 12px;
    text-decoration: none;
    color: inherit;
    transition: border-color 0.15s;
  }

  .wiki-recent-item:hover {
    border-color: var(--gold);
  }

  .wiki-recent-thumb {
    width: 80px;
    height: 52px;
    overflow: hidden;
    border: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
    background: rgba(0, 0, 0, 0.3);
  }

  .wiki-recent-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    image-rendering: pixelated;
  }

  .wiki-recent-fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    font-size: 24px;
  }

  .wiki-recent-body h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--gold-light);
  }

  .wiki-recent-body p {
    margin: 4px 0 0;
    font-size: 11px;
    line-height: 1.45;
    color: var(--muted);
  }

  .wiki-recent-item time {
    font-size: 10px;
    color: var(--muted);
    white-space: nowrap;
  }

  @media (max-width: 640px) {
    .wiki-recent-item {
      grid-template-columns: 64px 1fr;
    }

    .wiki-recent-item time {
      grid-column: 2;
    }
  }

  .wiki-browse-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 14px;
    padding: 12px 16px;
    border: 1px solid color-mix(in oklab, var(--gold) 45%, transparent);
    background: color-mix(in oklab, var(--gold) 8%, transparent);
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    text-decoration: none;
    color: var(--gold);
    transition: background 0.15s;
  }

  .wiki-browse-btn:hover {
    background: color-mix(in oklab, var(--gold) 15%, transparent);
  }
</style>
