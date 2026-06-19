<script lang="ts">
  import WikiArticleBody from '$lib/components/WikiArticleBody.svelte';
  import { formatDate } from '$lib/wiki';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();
  let article = $derived(data.article);
</script>

<svelte:head>
  <title>{article.title} — Solstead Wiki</title>
  <meta name="description" content={article.summary} />
  <link rel="canonical" href="https://solstead.xyz/wiki/{article.slug}/" />
</svelte:head>

<article class="wiki-article ornate-panel">
  <header class="wiki-article-header">
    {#if article.thumbnail}
      <img src={article.thumbnail} alt="" class="wiki-article-thumb" />
    {/if}
    <div>
      <p class="wiki-article-kicker">Solstead Wiki</p>
      <h1>{article.title}</h1>
      <p class="wiki-article-summary">{article.summary}</p>
      <p class="wiki-article-meta">Last updated {formatDate(article.updatedAt)}</p>
    </div>
  </header>

  <WikiArticleBody blocks={article.blocks} />

  {#if article.externalLink}
    <a href={article.externalLink} class="wiki-external-link">Open full database →</a>
  {/if}
</article>

<style>
  .ornate-panel {
    position: relative;
    padding: 20px 22px;
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

  .wiki-article-header {
    display: flex;
    gap: 16px;
    margin-bottom: 16px;
    padding-bottom: 16px;
    border-bottom: 1px solid rgba(90, 64, 48, 0.35);
  }

  .wiki-article-thumb {
    width: 72px;
    height: 72px;
    object-fit: cover;
    image-rendering: pixelated;
    border: 1px solid color-mix(in oklab, var(--gold) 30%, transparent);
    flex-shrink: 0;
  }

  .wiki-article-kicker {
    margin: 0;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 9px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .wiki-article-header h1 {
    margin: 6px 0 0;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: clamp(20px, 3vw, 26px);
    font-weight: 600;
    color: var(--gold-light);
  }

  .wiki-article-summary {
    margin: 8px 0 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-soft);
  }

  .wiki-article-meta {
    margin: 8px 0 0;
    font-size: 10px;
    color: var(--muted);
  }

  .wiki-external-link {
    display: inline-flex;
    margin-top: 16px;
    padding: 10px 16px;
    border: 1px solid color-mix(in oklab, var(--gold) 45%, transparent);
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    text-decoration: none;
    color: var(--gold);
    transition: background 0.15s;
  }

  .wiki-external-link:hover {
    background: rgba(212, 168, 68, 0.1);
  }
</style>
