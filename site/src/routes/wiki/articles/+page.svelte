<script lang="ts">
  import { ALL_ARTICLES, formatDate, searchArticles } from '$lib/wiki';

  let query = $state('');
  let articles = $derived(
    query.trim()
      ? searchArticles(query, 100)
      : [...ALL_ARTICLES].sort((a, b) => a.title.localeCompare(b.title)),
  );
</script>

<svelte:head>
  <title>All Articles — Solstead Wiki</title>
  <meta name="description" content="Browse every Solstead wiki article — quests, guides, regions, combat, and more." />
</svelte:head>

<section class="wiki-articles ornate-panel">
  <header>
    <h1>All Articles</h1>
    <p>{articles.length} articles {query.trim() ? `matching “${query}”` : 'in the wiki'}</p>
    <input
      type="search"
      class="wiki-articles-search"
      placeholder="Filter articles..."
      bind:value={query}
      aria-label="Filter articles"
    />
  </header>

  <ul class="wiki-articles-list">
    {#each articles as article}
      <li>
        <a href="/wiki/{article.slug}">
          <span class="wiki-articles-icon" aria-hidden="true">{article.icon ?? '📄'}</span>
          <span class="wiki-articles-body">
            <strong>{article.title}</strong>
            <span>{article.summary}</span>
          </span>
          <time datetime={article.updatedAt}>{formatDate(article.updatedAt)}</time>
        </a>
      </li>
    {/each}
  </ul>
</section>

<style>
  .ornate-panel {
    position: relative;
    padding: 20px 22px;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background:
      linear-gradient(180deg, rgba(22, 16, 12, 0.95), rgba(14, 10, 8, 0.98)),
      var(--panel);
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

  header h1 {
    margin: 0;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 22px;
    color: var(--gold-light);
  }

  header p {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--muted);
  }

  .wiki-articles-search {
    width: 100%;
    max-width: 360px;
    margin-top: 12px;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background: rgba(8, 6, 4, 0.88);
    padding: 9px 12px;
    font-size: 13px;
    color: var(--text);
    outline: none;
  }

  .wiki-articles-list {
    margin: 16px 0 0;
    padding: 0;
    list-style: none;
  }

  .wiki-articles-list a {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    gap: 10px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid rgba(90, 64, 48, 0.3);
    text-decoration: none;
    color: inherit;
    transition: color 0.15s;
  }

  .wiki-articles-list a:hover strong {
    color: var(--gold-light);
  }

  .wiki-articles-icon {
    font-size: 16px;
    text-align: center;
  }

  .wiki-articles-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .wiki-articles-body strong {
    font-size: 13px;
    color: var(--text);
  }

  .wiki-articles-body span {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .wiki-articles-list time {
    font-size: 10px;
    color: var(--muted);
    white-space: nowrap;
  }
</style>
