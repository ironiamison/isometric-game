<script lang="ts">
  import type { WikiBlock } from '$lib/wiki/types';

  let { blocks }: { blocks: WikiBlock[] } = $props();
</script>

<div class="wiki-body">
  {#each blocks as block}
    {#if block.type === 'p'}
      <p>{block.text}</p>
    {:else if block.type === 'h2'}
      <h2>{block.text}</h2>
    {:else if block.type === 'ul'}
      <ul>
        {#each block.items as item}
          <li>{item}</li>
        {/each}
      </ul>
    {:else if block.type === 'html'}
      {@html block.html}
    {:else if block.type === 'link'}
      <a href={block.href} class="wiki-inline-link">{block.label}</a>
    {/if}
  {/each}
</div>

<style>
  .wiki-body :global(p) {
    margin: 0 0 12px;
    font-size: 14px;
    line-height: 1.65;
    color: var(--text-soft);
  }

  .wiki-body :global(h2) {
    margin: 20px 0 8px;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 14px;
    letter-spacing: 0.08em;
    color: var(--gold-light);
  }

  .wiki-body :global(h2:first-child) {
    margin-top: 0;
  }

  .wiki-body :global(ul) {
    margin: 0 0 14px;
    padding-left: 18px;
  }

  .wiki-body :global(li) {
    margin-bottom: 6px;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-soft);
  }

  .wiki-body :global(ul.wiki-list li) {
    list-style: disc;
  }

  .wiki-body :global(.wiki-inline-link) {
    display: inline-flex;
    margin: 8px 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--gold);
    text-decoration: none;
  }

  .wiki-body :global(ul.wiki-list li a) {
    color: var(--gold);
    text-decoration: none;
  }

  .wiki-body :global(ul.wiki-list li a:hover) {
    text-decoration: underline;
  }

  .wiki-body :global(.wiki-table-wrap) {
    margin: 0 0 16px;
    max-height: 480px;
    overflow: auto;
    border: 1px solid rgba(90, 64, 48, 0.35);
    border-radius: 4px;
  }

  .wiki-body :global(.wiki-table) {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  .wiki-body :global(.wiki-table thead) {
    position: sticky;
    top: 0;
    background: rgba(22, 16, 12, 0.98);
    z-index: 1;
  }

  .wiki-body :global(.wiki-table th) {
    padding: 8px 10px;
    text-align: left;
    font-family: 'Cinzel', var(--font-display), serif;
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--gold);
    border-bottom: 1px solid rgba(90, 64, 48, 0.35);
  }

  .wiki-body :global(.wiki-table td) {
    padding: 7px 10px;
    color: var(--text-soft);
    border-bottom: 1px solid rgba(90, 64, 48, 0.2);
    vertical-align: top;
  }

  .wiki-body :global(.wiki-table tbody tr:hover) {
    background: rgba(212, 168, 68, 0.05);
  }

  .wiki-body :global(.wiki-table a) {
    color: var(--gold);
    text-decoration: none;
    font-weight: 600;
  }

  .wiki-body :global(.wiki-table a:hover) {
    text-decoration: underline;
  }
</style>
