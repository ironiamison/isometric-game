<script lang="ts">
  import { itemSpriteUrl, resolveItemSprite } from '$lib/item-sprite';

  let { sprite, alt = '' }: { sprite: string; alt?: string } = $props();

  let broken = $state(false);
  let src = $derived(itemSpriteUrl(resolveItemSprite(sprite)));
</script>

{#if !broken}
  <img {src} {alt} class="item-sprite" loading="lazy" decoding="async" onerror={() => (broken = true)} />
{:else}
  <div class="item-sprite item-sprite-fallback" aria-hidden="true">?</div>
{/if}

<style>
  .item-sprite {
    width: 40px;
    height: 40px;
    object-fit: contain;
    image-rendering: pixelated;
    flex-shrink: 0;
  }

  .item-sprite-fallback {
    display: grid;
    place-items: center;
    border: 1px solid var(--panel-border);
    background: var(--panel-soft);
    color: var(--muted);
    font-family: var(--font-display);
    font-size: 14px;
  }
</style>
