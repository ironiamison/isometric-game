<script lang="ts">
  import {
    hubMarkers,
    mapPois,
    mapRegionLabels,
    playerMapPosition,
    poiKindIcons,
    poiKindLabels,
    poiMatchesFilter,
    regionLabelVisible,
    hubMatchesFilter,
    getRegion,
    type MapFilterId,
    type MapPoi,
  } from '$lib/world-guide';

  let {
    selectedId = $bindable<string | null>(null),
    selectedRegionId = $bindable<string>('verdant'),
    variant = 'card',
    filters = $bindable<Record<MapFilterId, boolean> | undefined>(undefined),
    showBuiltInLegend = true,
    showSelectionBar = true,
    showPlayer = false,
    zoom = $bindable(1),
    panX = $bindable(0),
    panY = $bindable(0),
    centerRequest = $bindable<{ x: number; y: number; zoom?: number; n?: number } | null>(null),
  } = $props();

  const kindColors: Record<MapPoi['kind'], string> = {
    port: '#5a8aaa',
    town: '#d4a844',
    dungeon: '#c45a3a',
    landmark: '#e8c84a',
    travel: '#a88850',
    pvp: '#b04040',
    resource: '#7a9a5f',
  };

  let dragging = $state(false);
  let dragStart = $state({ x: 0, y: 0, panX: 0, panY: 0 });
  let hoveredPoiId = $state<string | null>(null);
  let viewportEl = $state<HTMLDivElement | null>(null);

  const MIN_ZOOM = 1;
  const MAX_ZOOM = 2.75;
  const activeFilters = $derived(filters ?? null);

  let visiblePois = $derived(
    activeFilters ? mapPois.filter((p) => poiMatchesFilter(p, activeFilters)) : mapPois,
  );
  let visibleHubs = $derived(
    activeFilters ? hubMarkers.filter((h) => hubMatchesFilter(h, activeFilters)) : hubMarkers,
  );

  let selectedPoi = $derived(mapPois.find((p) => p.id === selectedId) ?? null);
  let activeRegion = $derived(getRegion(selectedRegionId));

  $effect(() => {
    const req = centerRequest;
    const el = viewportEl;
    if (!req || !el) return;
    queueMicrotask(() => {
      if (!viewportEl) return;
      focusPoint(req.x, req.y, req.zoom ?? 1.75);
    });
  });

  function focusPoint(x: number, y: number, z = 1.75) {
    zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
    if (!viewportEl) return;
    const w = viewportEl.clientWidth;
    const h = viewportEl.clientHeight;
    panX = ((50 - x) / 100) * w * 0.55 * (zoom - 0.5);
    panY = ((50 - y) / 100) * h * 0.55 * (zoom - 0.5);
  }

  function selectPoi(poi: MapPoi, e?: Event) {
    e?.stopPropagation();
    selectedRegionId = poi.regionId;
    selectedId = poi.id;
    focusPoint(poi.x, poi.y, 2.1);
  }

  function selectRegion(regionId: string, e?: Event) {
    e?.stopPropagation();
    selectedRegionId = regionId;
    selectedId = null;
    const label = mapRegionLabels.find((l) => l.regionId === regionId);
    if (label) focusPoint(label.x, label.y, 1.6);
  }

  function zoomIn() {
    zoom = Math.min(MAX_ZOOM, +(zoom + 0.3).toFixed(2));
  }

  function zoomOut() {
    const next = Math.max(MIN_ZOOM, +(zoom - 0.3).toFixed(2));
    zoom = next;
    if (next === MIN_ZOOM) {
      panX = 0;
      panY = 0;
    }
  }

  function resetView() {
    zoom = 1;
    panX = 0;
    panY = 0;
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY < 0) zoomIn();
    else zoomOut();
  }

  function onPointerDown(e: PointerEvent) {
    if (zoom <= 1) return;
    const target = e.target as HTMLElement;
    if (target.closest('button, a, .map-control-btn')) return;
    dragging = true;
    dragStart = { x: e.clientX, y: e.clientY, panX, panY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    panX = dragStart.panX + (e.clientX - dragStart.x);
    panY = dragStart.panY + (e.clientY - dragStart.y);
  }

  function onPointerUp(e: PointerEvent) {
    dragging = false;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
  }
</script>

<div class="world-map-frame {variant === 'explorer' ? 'is-explorer' : ''}">
  {#if variant === 'card'}
    <span class="frame-corner frame-corner-tl" aria-hidden="true"></span>
    <span class="frame-corner frame-corner-tr" aria-hidden="true"></span>
    <span class="frame-corner frame-corner-bl" aria-hidden="true"></span>
    <span class="frame-corner frame-corner-br" aria-hidden="true"></span>
  {/if}

  <div
    bind:this={viewportEl}
    class="map-viewport {dragging ? 'is-dragging' : ''} {zoom > 1 ? 'can-pan' : ''}"
    onwheel={onWheel}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    role="application"
    aria-label="Interactive world map"
  >
    <div class="map-canvas" style="transform: translate({panX}px, {panY}px) scale({zoom});">
      <img
        src="/world/world-map.png"
        alt="Solstead overworld map"
        class="map-image"
        draggable="false"
      />
      <div class="map-vignette" aria-hidden="true"></div>

      {#each mapRegionLabels as label (label.regionId)}
        {@const region = getRegion(label.regionId)}
        {#if region && (!activeFilters || regionLabelVisible(label.regionId, activeFilters))}
          <button
            type="button"
            class="region-label {selectedRegionId === label.regionId && !selectedId
              ? 'is-active'
              : ''}"
            style="left: {label.x}%; top: {label.y}%;"
            onclick={(e) => selectRegion(label.regionId, e)}
            onpointerdown={(e) => e.stopPropagation()}
          >
            <span class="region-label-name">{region.name}</span>
            <span class="region-label-tag">{region.tagline}</span>
          </button>
        {/if}
      {/each}

      {#if showPlayer}
        <div
          class="player-marker"
          style="left: {playerMapPosition.x}%; top: {playerMapPosition.y}%;"
          aria-label="Your location"
        >
          <span class="player-pulse"></span>
          <span class="player-dot"></span>
        </div>
      {/if}

      {#each visibleHubs as hub (hub.id)}
        <button
          type="button"
          class="hub-pin hub-{hub.kind}"
          style="left: {hub.x}%; top: {hub.y}%;"
          title={hub.name}
          aria-label={hub.name}
          onclick={(e) => {
            e.stopPropagation();
            selectedRegionId = hub.regionId;
            selectedId = null;
            focusPoint(hub.x, hub.y, 1.85);
          }}
        >
          {hub.kind === 'bank' ? '🪙' : '🛍'}
        </button>
      {/each}

      {#each visiblePois as poi (poi.id)}
        <button
          type="button"
          class="poi-pin {selectedId === poi.id ? 'is-active' : ''}"
          style="left: {poi.x}%; top: {poi.y}%; --pin-color: {kindColors[poi.kind]};"
          title={poi.name}
          aria-label="{poi.name} — {poiKindLabels[poi.kind]}"
          onclick={(e) => selectPoi(poi, e)}
          onpointerdown={(e) => e.stopPropagation()}
          onmouseenter={() => (hoveredPoiId = poi.id)}
          onmouseleave={() => (hoveredPoiId = null)}
        >
          <span class="poi-pin-icon">{poiKindIcons[poi.kind]}</span>
        </button>
        {#if hoveredPoiId === poi.id || selectedId === poi.id}
          <div class="poi-tooltip" style="left: {poi.x}%; top: {poi.y}%;" aria-hidden="true">
            {poi.name}
          </div>
        {/if}
      {/each}
    </div>
  </div>

  <div class="compass" aria-hidden="true">
    <span class="compass-icon">⊕</span>
    <span>N</span>
  </div>

  <div class="map-controls {variant === 'explorer' ? 'controls-top' : ''}">
    <button type="button" class="map-control-btn" aria-label="Zoom in" onclick={zoomIn}>+</button>
    <button type="button" class="map-control-btn" aria-label="Zoom out" onclick={zoomOut}>−</button>
    <button type="button" class="map-control-btn" aria-label="Reset map view" onclick={resetView}>◎</button>
  </div>

  {#if showBuiltInLegend && variant === 'card'}
    <div class="map-legend">
      <p class="legend-title">Legend</p>
      <ul>
        {#each Object.entries(poiKindLabels) as [kind, label]}
          <li>
            <span class="legend-dot" style="background: {kindColors[kind as MapPoi['kind']]}"></span>
            {label}
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if showSelectionBar && variant === 'card'}
    {#if selectedPoi}
      <div class="map-selection-bar">
        <span class="selection-kind" style="color: {kindColors[selectedPoi.kind]}">
          {poiKindLabels[selectedPoi.kind]}
        </span>
        <strong>{selectedPoi.name}</strong>
        <span class="selection-blurb">{selectedPoi.blurb}</span>
      </div>
    {:else if activeRegion}
      <div class="map-selection-bar">
        <span class="selection-kind text-[var(--gold)]">Region</span>
        <strong>{activeRegion.name}</strong>
        <span class="selection-blurb">{activeRegion.tagline}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .world-map-frame {
    position: relative;
    overflow: hidden;
    border: 1px solid color-mix(in oklab, var(--gold) 55%, transparent);
    background: #0a0806;
    box-shadow:
      inset 0 0 80px rgba(0, 0, 0, 0.45),
      0 8px 32px rgba(0, 0, 0, 0.35);
  }

  .world-map-frame.is-explorer {
    height: 100%;
    border: none;
    box-shadow: none;
    border-radius: 0;
  }

  .world-map-frame.is-explorer .map-viewport {
    aspect-ratio: unset;
    height: 100%;
    min-height: 420px;
  }

  .frame-corner {
    position: absolute;
    z-index: 30;
    width: 14px;
    height: 14px;
    border-color: var(--gold);
    border-style: solid;
    pointer-events: none;
  }

  .frame-corner-tl {
    top: 6px;
    left: 6px;
    border-width: 2px 0 0 2px;
  }

  .frame-corner-tr {
    top: 6px;
    right: 6px;
    border-width: 2px 2px 0 0;
  }

  .frame-corner-bl {
    bottom: 6px;
    left: 6px;
    border-width: 0 0 2px 2px;
  }

  .frame-corner-br {
    bottom: 6px;
    right: 6px;
    border-width: 0 2px 2px 0;
  }

  .map-viewport {
    position: relative;
    aspect-ratio: 4 / 3;
    width: 100%;
    overflow: hidden;
    cursor: default;
    touch-action: none;
  }

  .map-viewport.can-pan {
    cursor: grab;
  }

  .map-viewport.is-dragging {
    cursor: grabbing;
  }

  .map-canvas {
    position: absolute;
    inset: 0;
    transform-origin: center center;
    transition: transform 0.15s ease-out;
    z-index: 2;
  }

  .map-viewport.is-dragging .map-canvas {
    transition: none;
  }

  .map-image {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    user-select: none;
    pointer-events: none;
  }

  .map-vignette {
    pointer-events: none;
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse at center, transparent 40%, rgba(8, 6, 4, 0.55) 100%),
      linear-gradient(to bottom, rgba(8, 6, 4, 0.25), transparent 30%, rgba(8, 6, 4, 0.35));
  }

  .region-label {
    position: absolute;
    z-index: 12;
    max-width: 140px;
    transform: translate(-50%, -100%);
    margin-top: -10px;
    border: 1px solid rgba(212, 168, 68, 0.35);
    background: rgba(10, 8, 6, 0.82);
    padding: 6px 10px;
    text-align: center;
    cursor: pointer;
    pointer-events: auto;
    transition:
      border-color 0.15s,
      background 0.15s,
      box-shadow 0.15s;
  }

  .region-label:hover,
  .region-label.is-active {
    border-color: var(--gold);
    background: rgba(18, 12, 8, 0.92);
    box-shadow: 0 0 16px rgba(212, 168, 68, 0.2);
  }

  .region-label-name {
    display: block;
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--gold-light);
  }

  .region-label-tag {
    display: block;
    margin-top: 2px;
    font-size: 10px;
    line-height: 1.3;
    color: var(--text-soft);
  }

  .player-marker {
    position: absolute;
    z-index: 16;
    transform: translate(-50%, -50%);
    width: 20px;
    height: 20px;
    pointer-events: none;
  }

  .player-pulse {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: rgba(74, 159, 212, 0.35);
    animation: pulse-ring 2s ease-out infinite;
  }

  .player-dot {
    position: absolute;
    inset: 5px;
    border-radius: 50%;
    background: #4a9fd4;
    border: 2px solid #fff;
    box-shadow: 0 0 10px rgba(74, 159, 212, 0.8);
  }

  @keyframes pulse-ring {
    0% {
      transform: scale(0.6);
      opacity: 0.9;
    }
    100% {
      transform: scale(2.2);
      opacity: 0;
    }
  }

  .hub-pin {
    position: absolute;
    z-index: 13;
    transform: translate(-50%, -50%);
    width: 20px;
    height: 20px;
    border: 1px solid rgba(8, 6, 4, 0.8);
    border-radius: 50%;
    background: rgba(18, 12, 8, 0.9);
    font-size: 10px;
    cursor: pointer;
    pointer-events: auto;
    transition: transform 0.15s;
  }

  .hub-pin:hover {
    transform: translate(-50%, -50%) scale(1.15);
  }

  .poi-pin {
    position: absolute;
    z-index: 14;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    transform: translate(-50%, -50%);
    border: 2px solid rgba(8, 6, 4, 0.85);
    border-radius: 50%;
    background: var(--pin-color);
    box-shadow: 0 0 10px color-mix(in oklab, var(--pin-color) 60%, transparent);
    cursor: pointer;
    pointer-events: auto;
    transition:
      transform 0.15s,
      box-shadow 0.15s;
  }

  .poi-pin:hover,
  .poi-pin.is-active {
    transform: translate(-50%, -50%) scale(1.2);
    border-color: var(--gold-light);
    box-shadow: 0 0 14px color-mix(in oklab, var(--pin-color) 80%, transparent);
    z-index: 20;
  }

  .poi-pin-icon {
    font-size: 11px;
    line-height: 1;
    filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.5));
  }

  .poi-tooltip {
    position: absolute;
    z-index: 19;
    transform: translate(-50%, calc(-100% - 18px));
    white-space: nowrap;
    border: 1px solid rgba(212, 168, 68, 0.4);
    background: rgba(10, 8, 6, 0.9);
    padding: 4px 8px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text);
    pointer-events: none;
  }

  .compass {
    position: absolute;
    top: 12px;
    right: 12px;
    z-index: 25;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    color: var(--gold);
    opacity: 0.75;
    pointer-events: none;
  }

  .compass-icon {
    font-size: 18px;
    line-height: 1;
  }

  .compass span {
    font-family: var(--font-display);
    font-size: 8px;
    letter-spacing: 0.2em;
  }

  .map-controls {
    position: absolute;
    right: 12px;
    bottom: 12px;
    z-index: 25;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .map-controls.controls-top {
    top: 12px;
    bottom: auto;
  }

  .map-control-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 1px solid color-mix(in oklab, var(--gold) 45%, transparent);
    background: rgba(10, 8, 6, 0.88);
    color: var(--gold);
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    font-family: var(--font-body);
    transition:
      background 0.15s,
      border-color 0.15s;
  }

  .map-control-btn:hover {
    background: rgba(20, 14, 10, 0.95);
    border-color: var(--gold);
  }

  .map-legend {
    position: absolute;
    left: 12px;
    bottom: 12px;
    z-index: 25;
    min-width: 120px;
    border: 1px solid color-mix(in oklab, var(--gold) 30%, transparent);
    background: rgba(10, 8, 6, 0.88);
    padding: 8px 10px;
  }

  .legend-title {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-size: 8px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--gold);
  }

  .map-legend ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .map-legend li {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-soft);
  }

  .map-legend li + li {
    margin-top: 4px;
  }

  .legend-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .map-selection-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    border-top: 1px solid color-mix(in oklab, var(--gold) 25%, transparent);
    background: rgba(12, 8, 6, 0.95);
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-soft);
  }

  .selection-kind {
    font-family: var(--font-display);
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .map-selection-bar strong {
    color: var(--gold-light);
    font-family: var(--font-display);
    font-size: 11px;
    letter-spacing: 0.06em;
  }

  .selection-blurb {
    flex: 1 1 100%;
    font-size: 11px;
    line-height: 1.45;
  }

  @media (max-width: 640px) {
    .region-label {
      max-width: 100px;
      padding: 4px 6px;
    }

    .region-label-name {
      font-size: 8px;
    }

    .region-label-tag {
      display: none;
    }

    .map-legend {
      display: none;
    }
  }
</style>
