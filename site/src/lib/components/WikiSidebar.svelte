<script lang="ts">
  import { goto } from '$app/navigation';
  import { WIKI_NAV, searchArticles } from '$lib/wiki';

  let { currentSlug = 'welcome' }: { currentSlug?: string } = $props();

  let query = $state('');
  let results = $derived(searchArticles(query, 8));
  let open = $state(false);

  function go(slug: string) {
    query = '';
    open = false;
    goto(`/wiki/${slug}`);
  }

  function onSearchInput() {
    open = query.trim().length > 0;
  }
</script>

<aside class="wiki-sidebar">
  <div class="wiki-search-wrap">
    <span class="wiki-search-icon" aria-hidden="true">⌕</span>
    <input
      type="search"
      placeholder="Search the wiki..."
      bind:value={query}
      oninput={onSearchInput}
      onfocus={() => (open = query.trim().length > 0)}
      class="wiki-search"
      aria-label="Search wiki"
    />
    {#if open && results.length > 0}
      <ul class="wiki-search-results">
        {#each results as r}
          <li>
            <button type="button" onclick={() => go(r.slug)}>{r.title}</button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <nav class="wiki-nav">
    {#each WIKI_NAV as group}
      <div class="wiki-nav-group">
        <p class="wiki-nav-heading">{group.label}</p>
        <ul>
          {#each group.links as link}
            <li>
              <a
                href="/wiki/{link.slug}"
                class="wiki-nav-link {currentSlug === link.slug ? 'is-active' : ''}"
              >
                {#if link.icon}<span aria-hidden="true">{link.icon}</span>{/if}
                {link.label}
              </a>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  </nav>

  <div class="wiki-sidebar-cta">
    <p>Can't find something?</p>
    <a href="/wiki/support" class="wiki-cta-sm">Contact Support</a>
  </div>
</aside>

<style>
  .wiki-sidebar {
    display: none;
  }

  @media (min-width: 1024px) {
    .wiki-sidebar {
      display: block;
    }
  }

  .wiki-search-wrap {
    position: relative;
    margin-bottom: 14px;
  }

  .wiki-search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--gold);
    font-size: 14px;
    pointer-events: none;
  }

  .wiki-search {
    width: 100%;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background: rgba(10, 8, 6, 0.88);
    padding: 9px 10px 9px 30px;
    font-size: 12px;
    color: var(--text);
    outline: none;
  }

  .wiki-search:focus {
    border-color: var(--gold);
  }

  .wiki-search-results {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 20;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background: rgba(10, 8, 6, 0.98);
    max-height: 240px;
    overflow-y: auto;
  }

  .wiki-search-results button {
    display: block;
    width: 100%;
    border: none;
    background: transparent;
    padding: 8px 12px;
    text-align: left;
    font-size: 12px;
    color: var(--text-soft);
    cursor: pointer;
  }

  .wiki-search-results button:hover {
    background: rgba(212, 168, 68, 0.08);
    color: var(--gold-light);
  }

  .wiki-nav-group {
    margin-bottom: 14px;
  }

  .wiki-nav-heading {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .wiki-nav ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .wiki-nav-link {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    font-size: 12px;
    color: var(--text-soft);
    text-decoration: none;
    border-left: 2px solid transparent;
    transition:
      color 0.15s,
      border-color 0.15s,
      background 0.15s;
  }

  .wiki-nav-link:hover {
    color: var(--text);
    background: rgba(212, 168, 68, 0.05);
  }

  .wiki-nav-link.is-active {
    color: var(--gold-light);
    border-left-color: var(--gold);
    background: rgba(212, 168, 68, 0.1);
  }

  .wiki-sidebar-cta {
    margin-top: 16px;
    padding: 12px;
    border: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
    background: rgba(10, 8, 6, 0.7);
  }

  .wiki-sidebar-cta p {
    margin: 0 0 8px;
    font-size: 11px;
    color: var(--muted);
  }

  .wiki-cta-sm {
    display: block;
    border: 1px solid color-mix(in oklab, var(--gold) 40%, transparent);
    padding: 7px 10px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    text-align: center;
    text-decoration: none;
    color: var(--gold);
  }
</style>
