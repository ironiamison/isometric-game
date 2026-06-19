<script lang="ts">
  import WorldMap from '$lib/components/WorldMap.svelte';
  import {
    defaultMapFilters,
    getPoi,
    getRegion,
    mapLegendItems,
    mapPercentToTile,
    mapRegionLabels,
    playerMapPosition,
    type MapFilterId,
  } from '$lib/world-guide';
  import { RotateCcw } from '@lucide/svelte';

  let selectedPoiId = $state<string | null>(null);
  let selectedRegionId = $state('verdant');
  let filters = $state({ ...defaultMapFilters });
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let centerRequest = $state<{ x: number; y: number; zoom?: number } | null>(null);

  let selectedPoi = $derived(selectedPoiId ? getPoi(selectedPoiId) : null);
  let selectedRegion = $derived(getRegion(selectedRegionId));

  let displayCoords = $derived.by(() => {
    if (selectedPoi?.tileX != null) {
      return { tileX: selectedPoi.tileX, tileY: selectedPoi.tileY ?? 0 };
    }
    if (selectedPoi) {
      return mapPercentToTile(selectedPoi.x, selectedPoi.y);
    }
    const label = mapRegionLabels.find((l) => l.regionId === selectedRegionId);
    if (label) return mapPercentToTile(label.x, label.y);
    return { tileX: playerMapPosition.tileX, tileY: playerMapPosition.tileY };
  });

  let regionName = $derived(selectedRegion?.name ?? 'Unknown');

  function toggleFilter(id: MapFilterId) {
    filters = { ...filters, [id]: !filters[id] };
  }

  function clearFilters() {
    filters = { ...defaultMapFilters };
  }

  function centerOnPlayer() {
    centerRequest = {
      x: playerMapPosition.x,
      y: playerMapPosition.y,
      zoom: 2.2,
    };
    selectedRegionId = 'verdant';
    selectedPoiId = null;
  }

  function viewRegion() {
    const label = mapRegionLabels.find((l) => l.regionId === selectedRegionId);
    if (label) {
      centerRequest = { x: label.x, y: label.y, zoom: 2.1 };
    }
  }

  /** Minimap viewport box — rough approximation from pan/zoom */
  let minimapViewport = $derived({
    width: Math.max(20, 100 / zoom),
    height: Math.max(15, (100 / zoom) * 0.75),
    left: 50 - panX / 12 - 50 / zoom,
    top: 50 - panY / 12 - (50 / zoom) * 0.375,
  });
</script>

<div class="map-explorer">
  <aside class="map-sidebar">
    <div class="sidebar-panel">
      <h2 class="sidebar-title">Map Legend</h2>
      <ul class="legend-list">
        {#each mapLegendItems as item}
          <li>
            <span class="legend-icon" style={item.color ? `color: ${item.color}` : ''}>{item.icon}</span>
            <span>{item.label}</span>
          </li>
        {/each}
      </ul>
    </div>

    <div class="sidebar-panel">
      <h2 class="sidebar-title">Filters</h2>
      <ul class="filter-list">
        {#each mapLegendItems.filter((i) => i.filterId) as item}
          <li>
            <label>
              <input
                type="checkbox"
                checked={filters[item.filterId!]}
                onchange={() => toggleFilter(item.filterId!)}
              />
              <span>{item.label}</span>
            </label>
          </li>
        {/each}
      </ul>
      <button type="button" class="clear-filters" onclick={clearFilters}>
        <RotateCcw size={11} />
        Clear Filters
      </button>
    </div>

    <div class="sidebar-panel location-panel">
      <dl>
        <div>
          <dt>X</dt>
          <dd>{displayCoords.tileX}</dd>
        </div>
        <div>
          <dt>Y</dt>
          <dd>{displayCoords.tileY}</dd>
        </div>
      </dl>
      <p class="location-region">Region: <strong>{regionName}</strong></p>
    </div>
  </aside>

  <div class="map-stage">
    <WorldMap
      bind:selectedId={selectedPoiId}
      bind:selectedRegionId
      bind:filters
      bind:zoom
      bind:panX
      bind:panY
      bind:centerRequest
      variant="explorer"
      showBuiltInLegend={false}
      showSelectionBar={false}
      showPlayer={true}
    />

    {#if selectedRegion && !selectedPoi}
      <div class="region-popup">
        <div class="region-popup-text">
          <strong>{selectedRegion.name.toUpperCase()}</strong>
          <span>{selectedRegion.tagline}</span>
        </div>
        <button type="button" class="view-region-btn" onclick={viewRegion}>View Region</button>
      </div>
    {:else if selectedPoi}
      <div class="region-popup">
        <div class="region-popup-text">
          <strong>{selectedPoi.name.toUpperCase()}</strong>
          <span>{selectedPoi.blurb}</span>
        </div>
        <button
          type="button"
          class="view-region-btn"
          onclick={() => (selectedPoiId = null)}
        >
          Close
        </button>
      </div>
    {/if}

    <div class="minimap-wrap">
      <div class="minimap">
        <img src="/world/world-map.png" alt="" draggable="false" />
        <div
          class="minimap-viewport"
          style="left: {minimapViewport.left}%; top: {minimapViewport.top}%; width: {minimapViewport.width}%; height: {minimapViewport.height}%;"
        ></div>
        <div
          class="minimap-player"
          style="left: {playerMapPosition.x}%; top: {playerMapPosition.y}%;"
        ></div>
      </div>
      <button type="button" class="center-you-btn" onclick={centerOnPlayer}>
        <span class="center-dot"></span>
        Center on You
      </button>
    </div>
  </div>
</div>

<style>
  .map-explorer {
    display: grid;
    grid-template-columns: 1fr;
    gap: 0;
    min-height: calc(100dvh - 56px);
    margin: -1.5rem -1rem;
  }

  @media (min-width: 900px) {
    .map-explorer {
      grid-template-columns: 220px 1fr;
      margin: -2rem -2.5rem;
      min-height: calc(100dvh - 52px);
    }
  }

  .map-sidebar {
    display: none;
    flex-direction: column;
    gap: 0;
    border-right: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
    background: rgba(10, 8, 6, 0.92);
    padding: 12px;
  }

  @media (min-width: 900px) {
    .map-sidebar {
      display: flex;
    }
  }

  .sidebar-panel {
    padding: 12px 10px;
    border-bottom: 1px solid rgba(90, 64, 48, 0.35);
  }

  .sidebar-panel:last-child {
    border-bottom: none;
    margin-top: auto;
  }

  .sidebar-title {
    margin: 0 0 10px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .legend-list,
  .filter-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .legend-list li,
  .filter-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--text-soft);
  }

  .legend-list li + li,
  .filter-list li + li {
    margin-top: 6px;
  }

  .legend-icon {
    width: 16px;
    text-align: center;
    font-size: 12px;
    flex-shrink: 0;
  }

  .filter-list label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 11px;
    color: var(--text-soft);
  }

  .filter-list input {
    accent-color: var(--gold);
  }

  .clear-filters {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 12px;
    border: 1px solid rgba(90, 64, 48, 0.5);
    background: transparent;
    padding: 6px 10px;
    font-family: var(--font-display);
    font-size: 8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--muted);
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }

  .clear-filters:hover {
    color: var(--gold);
    border-color: var(--gold);
  }

  .location-panel dl {
    margin: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .location-panel dt {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--muted);
  }

  .location-panel dd {
    margin: 2px 0 0;
    font-family: var(--font-display);
    font-size: 13px;
    color: var(--text);
  }

  .location-region {
    margin: 10px 0 0;
    font-size: 11px;
    color: var(--text-soft);
  }

  .location-region strong {
    color: var(--gold-light);
  }

  .map-stage {
    position: relative;
    min-height: 420px;
    background: #060504;
  }

  @media (min-width: 900px) {
    .map-stage {
      min-height: calc(100dvh - 52px);
    }
  }

  .region-popup {
    position: absolute;
    left: 50%;
    bottom: 24px;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 16px;
    transform: translateX(-50%);
    max-width: min(520px, calc(100% - 32px));
    border: 1px solid color-mix(in oklab, var(--gold) 45%, transparent);
    background: rgba(10, 8, 6, 0.94);
    padding: 12px 16px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
  }

  .region-popup-text {
    flex: 1;
    min-width: 0;
  }

  .region-popup-text strong {
    display: block;
    font-family: var(--font-display);
    font-size: 11px;
    letter-spacing: 0.1em;
    color: var(--gold-light);
  }

  .region-popup-text span {
    display: block;
    margin-top: 4px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-soft);
  }

  .view-region-btn {
    flex-shrink: 0;
    border: 1px solid var(--gold);
    background: color-mix(in oklab, var(--gold) 15%, transparent);
    padding: 8px 14px;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--gold-light);
    cursor: pointer;
    transition: background 0.15s;
  }

  .view-region-btn:hover {
    background: color-mix(in oklab, var(--gold) 28%, transparent);
  }

  .minimap-wrap {
    position: absolute;
    right: 12px;
    bottom: 12px;
    z-index: 28;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
  }

  @media (max-width: 640px) {
    .minimap-wrap {
      display: none;
    }

    .region-popup {
      flex-direction: column;
      align-items: stretch;
      bottom: 12px;
    }
  }

  .minimap {
    position: relative;
    width: 120px;
    height: 90px;
    overflow: hidden;
    border: 1px solid color-mix(in oklab, var(--gold) 40%, transparent);
    background: #0a0806;
  }

  .minimap img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.85;
  }

  .minimap-viewport {
    position: absolute;
    border: 1px solid var(--gold-light);
    background: rgba(212, 168, 68, 0.08);
    pointer-events: none;
  }

  .minimap-player {
    position: absolute;
    width: 6px;
    height: 6px;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: #4a9fd4;
    border: 1px solid #fff;
    box-shadow: 0 0 4px rgba(74, 159, 212, 0.9);
    pointer-events: none;
  }

  .center-you-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid color-mix(in oklab, var(--gold) 35%, transparent);
    background: rgba(10, 8, 6, 0.92);
    padding: 6px 8px;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-soft);
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }

  .center-you-btn:hover {
    color: var(--gold);
    border-color: var(--gold);
  }

  .center-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4a9fd4;
  }
</style>
