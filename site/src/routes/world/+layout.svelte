<script lang="ts">
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import WorldSubNav from '$lib/components/WorldSubNav.svelte';

  let { children } = $props();
</script>

<div class="world-root">
  <div class="world-bg" aria-hidden="true"></div>
  <div class="world-bg-shade" aria-hidden="true"></div>

  <SiteHeader />
  <WorldSubNav />

  <main class="world-main">
    <div class="world-main-inner">
      {@render children()}
    </div>
  </main>
</div>

<style>
  .world-root {
    position: relative;
    min-height: 100dvh;
    background: #0a0806;
    color: var(--text);
  }

  .world-bg {
    pointer-events: none;
    position: fixed;
    inset: 0;
    z-index: 0;
    background: #0a0806 url('/world/world-bg.png') center center / cover no-repeat;
  }

  .world-bg-shade {
    pointer-events: none;
    position: fixed;
    inset: 0;
    z-index: 0;
    /* Light vignette only — keep the parchment map visible */
    background:
      linear-gradient(180deg, rgba(8, 6, 4, 0.45) 0%, rgba(8, 6, 4, 0.12) 42%, rgba(8, 6, 4, 0.38) 100%),
      radial-gradient(ellipse at center, transparent 30%, rgba(8, 6, 4, 0.35) 100%);
  }

  .world-main {
    position: relative;
    z-index: 5;
    padding: 16px 16px 32px;
    pointer-events: auto;
  }

  .world-main-inner {
    max-width: 1400px;
    margin: 0 auto;
  }

  @media (min-width: 768px) {
    .world-main {
      padding: 24px 32px 40px;
    }
  }
</style>
