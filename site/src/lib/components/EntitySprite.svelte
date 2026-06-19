<script lang="ts">
  import { entitySpriteUrl, resolveEntitySprite } from '$lib/item-sprite';

  let { sprite, alt = '' }: { sprite: string; alt?: string } = $props();

  let broken = $state(false);
  let src = $derived(entitySpriteUrl(resolveEntitySprite(sprite)));
</script>

{#if !broken}
  <img {src} {alt} class="entity-sprite" loading="lazy" decoding="async" onerror={() => (broken = true)} />
{:else}
  <div class="entity-sprite entity-sprite-fallback" aria-hidden="true">☠</div>
{/if}

<style>
  .entity-sprite {
    width: 48px;
    height: 48px;
    object-fit: contain;
    object-position: bottom center;
    image-rendering: pixelated;
    flex-shrink: 0;
  }

  .entity-sprite-fallback {
    display: grid;
    place-items: center;
    border: 1px solid var(--panel-border);
    background: rgba(180, 60, 60, 0.12);
    color: var(--ember);
    font-size: 18px;
  }
</style>
