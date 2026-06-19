<script lang="ts">
  import { page } from '$app/stores';
  import { WORLD_SUB_NAV, worldSubNavActive } from '$lib/site-nav';

  let pathname = $derived($page.url.pathname);
</script>

<nav class="world-subnav" aria-label="World section">
  <div class="world-subnav-inner">
    {#each WORLD_SUB_NAV as item}
      <a
        href={item.href}
        class="world-subnav-link {worldSubNavActive(pathname, item.href, item.exact)
          ? 'is-active'
          : ''}"
      >
        {item.label}
      </a>
    {/each}
  </div>
</nav>

<style>
  .world-subnav {
    position: sticky;
    top: 60px;
    z-index: 40;
    border-bottom: 1px solid rgba(90, 64, 48, 0.45);
    background: rgba(26, 18, 16, 0.92);
    backdrop-filter: blur(8px);
  }

  .world-subnav-inner {
    display: flex;
    align-items: center;
    gap: 2px;
    max-width: 1320px;
    margin: 0 auto;
    padding: 0 16px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .world-subnav-inner::-webkit-scrollbar {
    display: none;
  }

  @media (min-width: 768px) {
    .world-subnav-inner {
      padding: 0 32px;
      gap: 4px;
    }
  }

  .world-subnav-link {
    position: relative;
    flex-shrink: 0;
    padding: 10px 14px;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-decoration: none;
    color: rgba(196, 168, 130, 0.85);
    transition: color 0.15s;
    white-space: nowrap;
  }

  .world-subnav-link:hover {
    color: #f5e6c8;
  }

  .world-subnav-link.is-active {
    color: #d4a844;
  }

  .world-subnav-link.is-active::after {
    content: '';
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: 0;
    height: 2px;
    background: #d4a844;
  }
</style>
