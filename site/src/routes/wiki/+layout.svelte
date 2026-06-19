<script lang="ts">
  import { page } from '$app/stores';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import WikiRightPanel from '$lib/components/WikiRightPanel.svelte';
  import WikiSidebar from '$lib/components/WikiSidebar.svelte';

  let { children } = $props();

  let path = $derived($page.url.pathname);
  let currentSlug = $derived(
    path === '/wiki' || path === '/wiki/'
      ? 'welcome'
      : ($page.params.slug ?? 'welcome'),
  );
</script>

<div class="wiki-root">
  <div class="wiki-bg" aria-hidden="true"></div>
  <div class="wiki-shade" aria-hidden="true"></div>

  <SiteHeader />

  <div class="wiki-shell">
    <WikiSidebar {currentSlug} />
    <div class="wiki-main">
      {@render children()}
    </div>
    <WikiRightPanel />
  </div>

  <footer class="wiki-footer">
    <div class="wiki-footer-scene" aria-hidden="true"></div>
    <div class="wiki-footer-inner">
      <div>
        <p class="wiki-footer-copy">© 2024 Solstead</p>
        <p class="wiki-footer-tag">Made by players, for players.</p>
      </div>
      <div class="wiki-social">
        <a href="https://discord.gg/solstead" aria-label="Discord" title="Discord">◆</a>
        <a href="https://reddit.com/r/solstead" aria-label="Reddit" title="Reddit">●</a>
        <a href="https://x.com/solsteadsol" aria-label="X" title="X (@solsteadsol)">✕</a>
        <a href="https://youtube.com/@solstead" aria-label="YouTube" title="YouTube">▶</a>
      </div>
    </div>
  </footer>
</div>

<style>
  .wiki-root {
    position: relative;
    min-height: 100dvh;
    margin: 0;
    background: #0a0806;
    color: var(--text);
  }

  .wiki-bg {
    position: fixed;
    inset: 0;
    z-index: 0;
    background: #0a0806 url('/wiki/wiki-bg.png') center center / cover no-repeat;
    pointer-events: none;
  }

  .wiki-shade {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background:
      linear-gradient(180deg, rgba(8, 6, 4, 0.5) 0%, rgba(8, 6, 4, 0.15) 45%, rgba(8, 6, 4, 0.42) 100%),
      radial-gradient(ellipse at center, transparent 25%, rgba(8, 6, 4, 0.4) 100%);
  }

  .wiki-shell {
    position: relative;
    z-index: 1;
    display: grid;
    gap: 14px;
    max-width: 1320px;
    margin: 0 auto;
    padding: 16px 16px 0;
  }

  @media (min-width: 1024px) {
    .wiki-shell {
      grid-template-columns: 220px minmax(0, 1fr);
      padding: 20px 20px 0;
    }
  }

  @media (min-width: 1200px) {
    .wiki-shell {
      grid-template-columns: 220px minmax(0, 1fr) 240px;
    }
  }

  .wiki-main {
    min-width: 0;
  }

  .wiki-footer {
    position: relative;
    z-index: 1;
    margin-top: 32px;
    border-top: 1px solid color-mix(in oklab, var(--gold) 20%, transparent);
    background: rgba(6, 4, 3, 0.82);
    backdrop-filter: blur(6px);
    overflow: hidden;
  }

  .wiki-footer-scene {
    height: 72px;
    background:
      linear-gradient(180deg, transparent, rgba(6, 4, 3, 0.9)),
      linear-gradient(90deg, rgba(20, 14, 10, 0.8) 0%, transparent 30%, transparent 70%, rgba(20, 14, 10, 0.6) 100%);
    mask-image: linear-gradient(180deg, transparent, black 40%);
  }

  .wiki-footer-inner {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    max-width: 1320px;
    margin: 0 auto;
    padding: 0 20px 20px;
  }

  .wiki-footer-copy {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }

  .wiki-footer-tag {
    margin: 4px 0 0;
    font-size: 10px;
    color: color-mix(in oklab, var(--muted) 80%, transparent);
  }

  .wiki-social {
    display: flex;
    gap: 12px;
  }

  .wiki-social a {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    font-size: 11px;
    color: var(--gold);
    text-decoration: none;
    transition: background 0.15s;
  }

  .wiki-social a:hover {
    background: rgba(212, 168, 68, 0.1);
  }
</style>
