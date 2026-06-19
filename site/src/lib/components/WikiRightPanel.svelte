<script lang="ts">
  import { formatDate, popularArticles, recentlyUpdated } from '$lib/wiki';

  let popular = popularArticles();
  let updated = recentlyUpdated();
</script>

<aside class="wiki-right">
  <div class="wiki-panel">
    <h2 class="wiki-panel-title">Popular Articles</h2>
    <ul class="wiki-link-list">
      {#each popular as article}
        <li>
          <a href="/wiki/{article.slug}">
            <span class="wiki-link-icon" aria-hidden="true">{article.icon ?? '📄'}</span>
            {article.title}
          </a>
        </li>
      {/each}
    </ul>
  </div>

  <div class="wiki-panel">
    <h2 class="wiki-panel-title">Recently Updated</h2>
    <ul class="wiki-update-list">
      {#each updated as article}
        <li>
          <a href="/wiki/{article.slug}">{article.title}</a>
          <span class="wiki-update-date">Updated {formatDate(article.updatedAt)}</span>
        </li>
      {/each}
    </ul>
    <a href="/wiki/articles" class="wiki-panel-btn">View All Updates</a>
  </div>

  <div class="wiki-panel wiki-help">
    <h2 class="wiki-panel-title">Need Help?</h2>
    <p>Can't find what you're looking for? Our support team is here to help.</p>
    <a href="/wiki/support" class="wiki-panel-btn wiki-panel-btn-solid">Contact Support</a>
  </div>
</aside>

<style>
  .wiki-right {
    display: none;
    flex-direction: column;
    gap: 14px;
  }

  @media (min-width: 1200px) {
    .wiki-right {
      display: flex;
    }
  }

  .wiki-panel {
    position: relative;
    padding: 14px 16px;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background:
      linear-gradient(180deg, rgba(22, 16, 12, 0.95), rgba(14, 10, 8, 0.98)),
      var(--panel);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .wiki-panel::before,
  .wiki-panel::after {
    content: '';
    position: absolute;
    width: 10px;
    height: 10px;
    border-color: color-mix(in oklab, var(--gold) 50%, transparent);
    border-style: solid;
    pointer-events: none;
  }

  .wiki-panel::before {
    top: 5px;
    left: 5px;
    border-width: 1px 0 0 1px;
  }

  .wiki-panel::after {
    bottom: 5px;
    right: 5px;
    border-width: 0 1px 1px 0;
  }

  .wiki-panel-title {
    margin: 0 0 10px;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .wiki-link-list,
  .wiki-update-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .wiki-link-list a {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    font-size: 12px;
    color: var(--text-soft);
    text-decoration: none;
    border-bottom: 1px solid rgba(90, 64, 48, 0.25);
    transition: color 0.15s;
  }

  .wiki-link-list li:last-child a {
    border-bottom: none;
  }

  .wiki-link-list a:hover {
    color: var(--gold-light);
  }

  .wiki-link-icon {
    font-size: 13px;
    opacity: 0.85;
  }

  .wiki-update-list li {
    padding: 7px 0;
    border-bottom: 1px solid rgba(90, 64, 48, 0.25);
  }

  .wiki-update-list li:last-child {
    border-bottom: none;
  }

  .wiki-update-list a {
    display: block;
    font-size: 12px;
    color: var(--text-soft);
    text-decoration: none;
  }

  .wiki-update-list a:hover {
    color: var(--gold-light);
  }

  .wiki-update-date {
    display: block;
    margin-top: 2px;
    font-size: 10px;
    color: var(--muted);
  }

  .wiki-panel-btn {
    display: block;
    margin-top: 10px;
    padding: 8px 12px;
    border: 1px solid color-mix(in oklab, var(--gold) 40%, transparent);
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    text-align: center;
    text-decoration: none;
    color: var(--gold);
    transition: background 0.15s;
  }

  .wiki-panel-btn:hover {
    background: rgba(212, 168, 68, 0.08);
  }

  .wiki-panel-btn-solid {
    border-color: var(--gold);
    background: color-mix(in oklab, var(--gold) 15%, transparent);
  }

  .wiki-help p {
    margin: 0 0 10px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--muted);
  }
</style>
