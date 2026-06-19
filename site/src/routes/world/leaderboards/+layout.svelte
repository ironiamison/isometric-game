<script lang="ts">
  let { children } = $props();
</script>

<div class="lb-page">
  <div class="lb-bg" aria-hidden="true"></div>
  <div class="lb-bg-shade" aria-hidden="true"></div>
  <div class="lb-inner">
    {@render children()}
  </div>
</div>

<style>
  .lb-page {
    position: relative;
    margin: -16px -16px;
    min-height: calc(100dvh - 130px);
  }

  @media (min-width: 768px) {
    .lb-page {
      margin: -24px -32px;
      min-height: calc(100dvh - 130px);
    }
  }

  .lb-bg {
    position: fixed;
    inset: 0;
    z-index: 0;
    background: #0a0806 url('/world/leaderboard-bg.png') center bottom / cover no-repeat;
    pointer-events: none;
  }

  .lb-bg-shade {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background:
      linear-gradient(180deg, rgba(8, 6, 4, 0.88) 0%, rgba(8, 6, 4, 0.55) 18%, rgba(8, 6, 4, 0.35) 45%, rgba(8, 6, 4, 0.72) 100%),
      radial-gradient(900px 500px at 50% 0%, rgba(212, 168, 68, 0.08), transparent 60%);
  }

  .lb-inner {
    position: relative;
    z-index: 1;
    padding: 0 12px 24px;
  }

  @media (min-width: 768px) {
    .lb-inner {
      padding: 0 20px 32px;
    }
  }
</style>
