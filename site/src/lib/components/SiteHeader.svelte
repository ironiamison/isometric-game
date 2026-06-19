<script lang="ts">
  import { page } from '$app/stores';
  import { Menu, X } from '@lucide/svelte';
  import { SITE_NAV_LINKS } from '$lib/site-nav';

  let mobileOpen = $state(false);

  let pathname = $derived($page.url.pathname);
</script>

<svelte:head>
  <link
    rel="stylesheet"
    href="https://fonts.googleapis.com/css2?family=Cinzel:wght@500;600;700&family=Inter:wght@400;500;600&display=swap"
  />
</svelte:head>

<header class="site-header">
  <div class="site-header-inner">
    <a href="/play/index.html" class="site-brand">
      <img src="/play/assets/title/logo-nav.png" alt="Solstead" class="site-logo" height="32" />
    </a>

    <nav class="site-nav" aria-label="Site">
      {#each SITE_NAV_LINKS as link}
        <a
          href={link.href}
          class="site-nav-link {link.isActive(pathname) ? 'is-active' : ''}"
        >
          {link.label}
        </a>
      {/each}
    </nav>

    <div class="site-header-right">
      <div class="site-header-actions">
        <a href="/play/index.html" class="site-btn site-btn-outline">Login</a>
        <a href="/play/index.html" class="site-btn site-btn-play">Play Now</a>
      </div>

      <button
        type="button"
        class="site-menu-btn"
        aria-label="Toggle menu"
        aria-expanded={mobileOpen}
        onclick={() => (mobileOpen = !mobileOpen)}
      >
        {#if mobileOpen}
          <X size={18} />
        {:else}
          <Menu size={18} />
        {/if}
      </button>
    </div>
  </div>

  {#if mobileOpen}
    <nav class="site-mobile-nav" aria-label="Site mobile">
      {#each SITE_NAV_LINKS as link}
        <a
          href={link.href}
          class="site-mobile-link {link.isActive(pathname) ? 'is-active' : ''}"
          onclick={() => (mobileOpen = false)}
        >
          {link.label}
        </a>
      {/each}
      <div class="site-mobile-actions">
        <a href="/play/index.html" class="site-btn site-btn-outline">Login</a>
        <a href="/play/index.html" class="site-btn site-btn-play">Play Now</a>
      </div>
    </nav>
  {/if}
</header>

<style>
  .site-header {
    position: sticky;
    top: 0;
    z-index: 50;
    border-bottom: 1px solid rgba(201, 162, 39, 0.22);
    background: rgba(8, 6, 4, 0.94);
    backdrop-filter: blur(10px);
  }

  .site-header-inner {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    max-width: 1320px;
    margin: 0 auto;
    padding: 14px 20px;
    min-height: 60px;
  }

  @media (min-width: 768px) {
    .site-header-inner {
      padding: 14px 32px;
    }
  }

  .site-header-right {
    display: flex;
    align-items: center;
    gap: 10px;
    position: relative;
    z-index: 2;
  }

  .site-brand {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    text-decoration: none;
    position: relative;
    z-index: 2;
  }

  .site-logo {
    height: 32px;
    width: auto;
    image-rendering: pixelated;
    image-rendering: crisp-edges;
    filter: drop-shadow(0 1px 4px rgba(0, 0, 0, 0.6));
  }

  .site-nav {
    display: none;
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    align-items: center;
    justify-content: center;
    gap: 28px;
    white-space: nowrap;
    z-index: 1;
    pointer-events: none;
  }

  @media (min-width: 900px) {
    .site-nav {
      display: flex;
    }
  }

  .site-nav-link {
    pointer-events: auto;
    position: relative;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    text-decoration: none;
    color: #d4af37;
    opacity: 0.85;
    transition: opacity 0.15s;
  }

  .site-nav-link:hover,
  .site-nav-link.is-active {
    opacity: 1;
  }

  .site-nav-link.is-active::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    bottom: -6px;
    height: 1px;
    background: #c9a227;
  }

  .site-header-actions {
    display: none;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  @media (min-width: 640px) {
    .site-header-actions {
      display: flex;
    }
  }

  .site-btn {
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    text-decoration: none;
    padding: 10px 22px;
    border: 1px solid #c9a227;
    cursor: pointer;
    transition:
      background 0.15s,
      color 0.15s,
      filter 0.15s;
  }

  .site-btn-outline {
    background: transparent;
    color: #d4af37;
  }

  .site-btn-outline:hover {
    background: rgba(201, 162, 39, 0.12);
  }

  .site-btn-play {
    background: #c9a227;
    color: #1a1210;
    font-weight: 600;
  }

  .site-btn-play:hover {
    filter: brightness(1.08);
  }

  .site-menu-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: 1px solid rgba(201, 162, 39, 0.35);
    background: transparent;
    color: #d4af37;
    cursor: pointer;
  }

  @media (min-width: 900px) {
    .site-menu-btn {
      display: none;
    }
  }

  .site-mobile-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    border-top: 1px solid rgba(201, 162, 39, 0.15);
    padding: 12px 20px 16px;
  }

  @media (min-width: 900px) {
    .site-mobile-nav {
      display: none;
    }
  }

  .site-mobile-link {
    padding: 10px 8px;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 12px;
    font-weight: 500;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    text-decoration: none;
    color: rgba(212, 175, 55, 0.75);
  }

  .site-mobile-link.is-active {
    color: #e8c84a;
  }

  .site-mobile-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    padding-top: 12px;
    border-top: 1px solid rgba(201, 162, 39, 0.15);
  }

  .site-mobile-actions .site-btn {
    flex: 1;
    text-align: center;
    padding: 10px 12px;
  }
</style>
