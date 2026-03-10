import type { Chunk, Viewport, WorldCoord, MapObject, Wall, InteriorMap, SpriteRect, DevNote } from '@/types';
import {
  TILE_WIDTH,
  TILE_HEIGHT,
  CHUNK_SIZE,
  worldToScreen,
  chunkLocalToWorld,
  indexToLocal
} from './coords';
import { BitSet } from './BitSet';
import { tilesetLoader } from './TilesetLoader';
import { objectLoader } from './ObjectLoader';

/** Returns the atlas source X for the current animation frame. */
function getAnimatedSourceX(rect: SpriteRect, frames: number | undefined, fps: number | undefined): number {
  if (!frames || frames <= 1) return rect.x;
  const frameIndex = Math.floor(performance.now() / 1000 * (fps ?? 4)) % frames;
  return rect.x + frameIndex * rect.w;
}

export interface RenderOptions {
  showGrid: boolean;
  showChunkBounds: boolean;
  showCollision: boolean;
  showEntities: boolean;
  showMapObjects: boolean;
  showPortals: boolean;
  showNotes: boolean;
  visibleLayers: {
    ground: boolean;
    objects: boolean;
    overhead: boolean;
  };
}

const DEFAULT_RENDER_OPTIONS: RenderOptions = {
  showGrid: false,
  showChunkBounds: true,
  showCollision: false,
  showEntities: true,
  showMapObjects: true,
  showPortals: true,
  showNotes: true,
  visibleLayers: {
    ground: true,
    objects: true,
    overhead: true,
  },
};

// ─── Chunk Tile Cache ──────────────────────────────────────────────
// Pre-renders chunk tile layers to an offscreen canvas so the main
// render loop blits one image per chunk instead of ~3072 drawImage calls.

/** Offscreen canvas dimensions for a 32x32 isometric chunk at zoom=1 */
const CHUNK_CANVAS_WIDTH = CHUNK_SIZE * TILE_WIDTH;   // 2048
const CHUNK_CANVAS_HEIGHT = CHUNK_SIZE * TILE_HEIGHT + TILE_HEIGHT; // 1056
const CHUNK_CANVAS_OFFSET_X = CHUNK_CANVAS_WIDTH / 2; // 1024

interface ChunkCacheEntry {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  groundRef: number[];
  objectsRef: number[];
  overheadRef: number[];
  visibleGround: boolean;
  visibleObjects: boolean;
  visibleOverhead: boolean;
}

class ChunkTileCache {
  private cache = new Map<string, ChunkCacheEntry>();
  private accessOrder: string[] = [];
  private maxSize = 40;

  getOrRender(
    key: string,
    chunk: Chunk,
    visibleLayers: { ground: boolean; objects: boolean; overhead: boolean }
  ): ChunkCacheEntry {
    let entry = this.cache.get(key);

    // Check if the cached entry is still valid
    const isValid = entry &&
      entry.groundRef === chunk.layers.ground &&
      entry.objectsRef === chunk.layers.objects &&
      entry.overheadRef === chunk.layers.overhead &&
      entry.visibleGround === visibleLayers.ground &&
      entry.visibleObjects === visibleLayers.objects &&
      entry.visibleOverhead === visibleLayers.overhead;

    if (isValid) {
      this.touch(key);
      return entry!;
    }

    // Create or reuse canvas
    if (!entry) {
      const canvas = document.createElement('canvas');
      canvas.width = CHUNK_CANVAS_WIDTH;
      canvas.height = CHUNK_CANVAS_HEIGHT;
      const ctx = canvas.getContext('2d')!;
      ctx.imageSmoothingEnabled = false;
      entry = {
        canvas, ctx,
        groundRef: chunk.layers.ground,
        objectsRef: chunk.layers.objects,
        overheadRef: chunk.layers.overhead,
        visibleGround: visibleLayers.ground,
        visibleObjects: visibleLayers.objects,
        visibleOverhead: visibleLayers.overhead,
      };
      this.cache.set(key, entry);
    }

    // Render tiles to the offscreen canvas
    this.renderToCache(entry, chunk, visibleLayers);

    // Update tracked references
    entry.groundRef = chunk.layers.ground;
    entry.objectsRef = chunk.layers.objects;
    entry.overheadRef = chunk.layers.overhead;
    entry.visibleGround = visibleLayers.ground;
    entry.visibleObjects = visibleLayers.objects;
    entry.visibleOverhead = visibleLayers.overhead;

    this.touch(key);
    this.evict();
    return entry;
  }

  invalidate(key: string): void {
    this.cache.delete(key);
    const idx = this.accessOrder.indexOf(key);
    if (idx >= 0) this.accessOrder.splice(idx, 1);
  }

  clear(): void {
    this.cache.clear();
    this.accessOrder.length = 0;
  }

  private touch(key: string): void {
    const idx = this.accessOrder.indexOf(key);
    if (idx >= 0) this.accessOrder.splice(idx, 1);
    this.accessOrder.push(key);
  }

  private evict(): void {
    while (this.cache.size > this.maxSize) {
      const oldest = this.accessOrder.shift();
      if (oldest) this.cache.delete(oldest);
    }
  }

  private renderToCache(
    entry: ChunkCacheEntry,
    chunk: Chunk,
    visibleLayers: { ground: boolean; objects: boolean; overhead: boolean }
  ): void {
    const { ctx } = entry;
    ctx.clearRect(0, 0, CHUNK_CANVAS_WIDTH, CHUNK_CANVAS_HEIGHT);

    const halfTileW = TILE_WIDTH / 2;
    const halfTileH = TILE_HEIGHT / 2;

    for (let y = 0; y < CHUNK_SIZE; y++) {
      for (let x = 0; x < CHUNK_SIZE; x++) {
        const index = y * CHUNK_SIZE + x;

        // Local isometric position (zoom=1, no viewport)
        const localIsoX = (x - y) * halfTileW;
        const localIsoY = (x + y) * halfTileH;
        const drawX = localIsoX - halfTileW + CHUNK_CANVAS_OFFSET_X;
        const drawY = localIsoY;

        if (visibleLayers.ground) {
          const gid = chunk.layers.ground[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }

        if (visibleLayers.objects) {
          const gid = chunk.layers.objects[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }

        if (visibleLayers.overhead) {
          const gid = chunk.layers.overhead[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }
      }
    }
  }
}

// ─── Interior Tile Cache ───────────────────────────────────────────
// Same concept but for interior maps (single fixed-size grid).

interface InteriorCacheEntry {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  groundRef: number[];
  objectsRef: number[];
  overheadRef: number[];
  visibleGround: boolean;
  visibleObjects: boolean;
  visibleOverhead: boolean;
  width: number;
  height: number;
}

class InteriorTileCache {
  private entry: InteriorCacheEntry | null = null;

  getOrRender(
    interior: InteriorMap,
    visibleLayers: { ground: boolean; objects: boolean; overhead: boolean }
  ): InteriorCacheEntry {
    const e = this.entry;
    const isValid = e &&
      e.groundRef === interior.layers.ground &&
      e.objectsRef === interior.layers.objects &&
      e.overheadRef === interior.layers.overhead &&
      e.visibleGround === visibleLayers.ground &&
      e.visibleObjects === visibleLayers.objects &&
      e.visibleOverhead === visibleLayers.overhead &&
      e.width === interior.width &&
      e.height === interior.height;

    if (isValid) return e!;

    // Calculate canvas size for this interior
    const w = interior.width;
    const h = interior.height;
    const canvasW = (w + h) * (TILE_WIDTH / 2);
    const canvasH = (w + h) * (TILE_HEIGHT / 2) + TILE_HEIGHT;
    const offsetX = h * (TILE_WIDTH / 2);

    // Create or resize canvas
    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    if (e && e.canvas.width === canvasW && e.canvas.height === canvasH) {
      canvas = e.canvas;
      ctx = e.ctx;
    } else {
      canvas = document.createElement('canvas');
      canvas.width = canvasW;
      canvas.height = canvasH;
      ctx = canvas.getContext('2d')!;
      ctx.imageSmoothingEnabled = false;
    }

    ctx.clearRect(0, 0, canvasW, canvasH);

    const halfTileW = TILE_WIDTH / 2;
    const halfTileH = TILE_HEIGHT / 2;

    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const index = y * w + x;
        const localIsoX = (x - y) * halfTileW;
        const localIsoY = (x + y) * halfTileH;
        const drawX = localIsoX - halfTileW + offsetX;
        const drawY = localIsoY;

        if (visibleLayers.ground) {
          const gid = interior.layers.ground[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }
        if (visibleLayers.objects) {
          const gid = interior.layers.objects[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }
        if (visibleLayers.overhead) {
          const gid = interior.layers.overhead[index];
          if (gid > 0) tilesetLoader.drawTile(ctx, gid, drawX, drawY, 1);
        }
      }
    }

    this.entry = {
      canvas, ctx,
      groundRef: interior.layers.ground,
      objectsRef: interior.layers.objects,
      overheadRef: interior.layers.overhead,
      visibleGround: visibleLayers.ground,
      visibleObjects: visibleLayers.objects,
      visibleOverhead: visibleLayers.overhead,
      width: w,
      height: h,
    };

    return this.entry;
  }

  clear(): void {
    this.entry = null;
  }
}

// ─── Main Renderer ─────────────────────────────────────────────────

export class IsometricRenderer {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private options: RenderOptions = DEFAULT_RENDER_OPTIONS;
  private notes: DevNote[] = [];
  private selectedNoteId: string | null = null;
  private tileCache = new ChunkTileCache();
  private interiorCache = new InteriorTileCache();

  attach(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    if (this.ctx) {
      this.ctx.imageSmoothingEnabled = false;
    }
  }

  detach(): void {
    this.canvas = null;
    this.ctx = null;
  }

  setOptions(options: Partial<RenderOptions>): void {
    this.options = { ...this.options, ...options };
  }

  getOptions(): RenderOptions {
    return this.options;
  }

  setNotes(notes: DevNote[], selectedId: string | null): void {
    this.notes = notes;
    this.selectedNoteId = selectedId;
  }

  /** Invalidate the tile cache for a specific chunk (call after edits). */
  invalidateChunk(key: string): void {
    this.tileCache.invalidate(key);
  }

  /** Clear all tile caches (e.g. on tileset reload). */
  clearTileCache(): void {
    this.tileCache.clear();
    this.interiorCache.clear();
  }

  clear(): void {
    if (!this.ctx || !this.canvas) return;
    this.ctx.fillStyle = '#0c0e1a';
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
  }

  render(chunks: Chunk[], viewport: Viewport): void {
    if (!this.ctx || !this.canvas) return;

    this.clear();

    // Filter to only visible chunks before sorting
    const visibleChunks = chunks.filter(chunk => this.isChunkVisible(chunk, viewport));

    // Sort chunks for proper depth ordering (back to front)
    const sortedChunks = visibleChunks.sort((a, b) => {
      const sumA = a.coord.cx + a.coord.cy;
      const sumB = b.coord.cx + b.coord.cy;
      return sumA - sumB;
    });

    // ── Render tile layers (cached) ──
    for (const chunk of sortedChunks) {
      this.blitCachedChunk(chunk, viewport);
    }

    // ── Render overlays (dynamic, drawn every frame) ──
    if (this.options.showChunkBounds) {
      for (const chunk of sortedChunks) {
        this.renderChunkBounds(chunk, viewport);
      }
    }

    if (this.options.showMapObjects) {
      for (const chunk of sortedChunks) {
        this.renderMapObjectsAndWalls(chunk, viewport);
      }
    }

    if (this.options.showCollision) {
      for (const chunk of sortedChunks) {
        this.renderCollisionOverlay(chunk, viewport);
      }
    }

    if (this.options.showEntities) {
      for (const chunk of sortedChunks) {
        this.renderEntities(chunk, viewport);
      }
    }

    if (this.options.showPortals) {
      for (const chunk of sortedChunks) {
        this.renderPortals(chunk, viewport);
      }
    }

    for (const chunk of sortedChunks) {
      this.renderGatheringZones(chunk, viewport);
    }

    this.renderNotes(viewport);

    if (this.options.showGrid) {
      this.renderGrid(viewport);
    }
  }

  /** Blit a cached chunk's tile canvas onto the main canvas. */
  private blitCachedChunk(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    const key = `${chunk.coord.cx},${chunk.coord.cy}`;
    const cached = this.tileCache.getOrRender(key, chunk, this.options.visibleLayers);

    // Calculate where the offscreen canvas maps to on screen.
    // The chunk's world origin tile is at (cx*CHUNK_SIZE, cy*CHUNK_SIZE).
    // In iso space (before viewport): isoX = (wx-wy)*(TW/2), isoY = (wx+wy)*(TH/2)
    // The offscreen canvas top-left is offset by -CHUNK_CANVAS_OFFSET_X in iso X.
    const cx = chunk.coord.cx;
    const cy = chunk.coord.cy;
    const chunkIsoX = (cx - cy) * CHUNK_SIZE * (TILE_WIDTH / 2);
    const chunkIsoY = (cx + cy) * CHUNK_SIZE * (TILE_HEIGHT / 2);

    const destX = (chunkIsoX - CHUNK_CANVAS_OFFSET_X) * viewport.zoom + viewport.offsetX;
    const destY = chunkIsoY * viewport.zoom + viewport.offsetY;
    const destW = CHUNK_CANVAS_WIDTH * viewport.zoom;
    const destH = CHUNK_CANVAS_HEIGHT * viewport.zoom;

    this.ctx.drawImage(cached.canvas, destX, destY, destW, destH);
  }

  // Render an interior map (fixed-size, not chunk-based)
  renderInterior(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx || !this.canvas) return;

    this.clear();

    // ── Blit cached interior tiles ──
    const cached = this.interiorCache.getOrRender(interior, this.options.visibleLayers);
    const offsetX = interior.height * (TILE_WIDTH / 2);
    const canvasW = cached.canvas.width;
    const canvasH = cached.canvas.height;

    // Interior world origin is (0, 0). In iso: isoX=0, isoY=0.
    // Offscreen canvas top-left is at (-offsetX, 0) in iso space.
    const destX = -offsetX * viewport.zoom + viewport.offsetX;
    const destY = viewport.offsetY;
    const destW = canvasW * viewport.zoom;
    const destH = canvasH * viewport.zoom;

    this.ctx.drawImage(cached.canvas, destX, destY, destW, destH);

    // Render interior bounds
    this.renderInteriorBounds(interior, viewport);

    // Render map objects and walls
    if (this.options.showMapObjects) {
      this.renderInteriorObjectsAndWalls(interior, viewport);
    }

    // Render collision overlay
    if (this.options.showCollision) {
      this.renderInteriorCollision(interior, viewport);
    }

    // Render entities
    if (this.options.showEntities) {
      this.renderInteriorEntities(interior, viewport);
    }

    // Render spawn points
    this.renderInteriorSpawnPoints(interior, viewport);

    // Render exit portals
    this.renderInteriorExitPortals(interior, viewport);

    // Render dev notes
    this.renderNotes(viewport);

    if (this.options.showGrid) {
      this.renderGrid(viewport);
    }
  }

  private renderInteriorBounds(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    const corners: WorldCoord[] = [
      { wx: 0, wy: 0 },
      { wx: interior.width, wy: 0 },
      { wx: interior.width, wy: interior.height },
      { wx: 0, wy: interior.height },
    ];

    const screenCorners = corners.map((c) => worldToScreen(c, viewport));

    this.ctx.strokeStyle = interior.dirty ? '#ff6b6b' : '#4ecdc4';
    this.ctx.lineWidth = 3;
    this.ctx.beginPath();
    this.ctx.moveTo(screenCorners[0].sx, screenCorners[0].sy);
    for (let i = 1; i < screenCorners.length; i++) {
      this.ctx.lineTo(screenCorners[i].sx, screenCorners[i].sy);
    }
    this.ctx.closePath();
    this.ctx.stroke();

    // Draw interior name label
    const centerWorld: WorldCoord = {
      wx: interior.width / 2,
      wy: interior.height / 2,
    };
    const centerScreen = worldToScreen(centerWorld, viewport);

    this.ctx.fillStyle = '#ffffff';
    this.ctx.font = `bold ${14 * viewport.zoom}px monospace`;
    this.ctx.textAlign = 'center';
    this.ctx.textBaseline = 'middle';
    this.ctx.fillText(interior.name, centerScreen.sx, centerScreen.sy - 20 * viewport.zoom);
  }

  private renderInteriorCollision(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    const bitset = new BitSet(interior.width * interior.height);
    bitset.setRaw(interior.collision);

    this.ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';

    for (let i = 0; i < interior.width * interior.height; i++) {
      if (bitset.get(i)) {
        const x = i % interior.width;
        const y = Math.floor(i / interior.width);
        const worldCoord: WorldCoord = { wx: x, wy: y };
        const screen = worldToScreen(worldCoord, viewport);

        const hw = (TILE_WIDTH / 2) * viewport.zoom;
        const hh = (TILE_HEIGHT / 2) * viewport.zoom;

        this.ctx.beginPath();
        this.ctx.moveTo(screen.sx, screen.sy);
        this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
        this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
        this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
        this.ctx.closePath();
        this.ctx.fill();
      }
    }
  }

  private renderInteriorObjectsAndWalls(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    type Renderable =
      | { type: 'object'; obj: MapObject }
      | { type: 'wall'; wall: Wall };

    const renderables: Array<{ depth: number; item: Renderable }> = [];

    for (const obj of interior.mapObjects) {
      renderables.push({
        depth: obj.x + obj.y,
        item: { type: 'object', obj }
      });
    }

    for (const wall of interior.walls) {
      renderables.push({
        depth: wall.x + wall.y,
        item: { type: 'wall', wall }
      });
    }

    renderables.sort((a, b) => a.depth - b.depth);

    for (const { item } of renderables) {
      if (item.type === 'object') {
        this.renderInteriorMapObject(item.obj, viewport);
      } else {
        this.renderInteriorWall(item.wall, viewport);
      }
    }
  }

  private renderInteriorMapObject(obj: MapObject, viewport: Viewport): void {
    if (!this.ctx) return;

    const worldCoord: WorldCoord = { wx: obj.x, wy: obj.y };
    const screen = worldToScreen(worldCoord, viewport);

    const objDef = objectLoader.getObject(objectLoader.gidToId(obj.gid));

    if (objDef?.image) {
      const r = objDef.atlasRect;
      const spriteW = r ? r.w : obj.width;
      const spriteH = r ? r.h : obj.height;
      const scaledWidth = spriteW * viewport.zoom;
      const scaledHeight = spriteH * viewport.zoom;

      const drawX = screen.sx - scaledWidth / 2;
      const drawY = screen.sy + TILE_HEIGHT * viewport.zoom - scaledHeight;

      const srcX = r ? getAnimatedSourceX(r, objDef.frames, objDef.fps) : 0;
      this.ctx.drawImage(
        objDef.image,
        srcX,
        r ? r.y : 0,
        r ? r.w : objDef.image.width,
        r ? r.h : objDef.image.height,
        drawX,
        drawY,
        scaledWidth,
        scaledHeight
      );
    }
  }

  private renderInteriorWall(wall: Wall, viewport: Viewport): void {
    if (!this.ctx) return;

    const worldCoord: WorldCoord = { wx: wall.x, wy: wall.y };
    const screen = worldToScreen(worldCoord, viewport);

    const objDef = objectLoader.getWallByGid(wall.gid);

    if (objDef?.image) {
      const r = objDef.atlasRect;
      const spriteW = r ? r.w : objDef.image.width;
      const spriteH = r ? r.h : objDef.image.height;
      const scaledWidth = spriteW * viewport.zoom;
      const scaledHeight = spriteH * viewport.zoom;

      const bottomVertexX = screen.sx;
      const bottomVertexY = screen.sy + TILE_HEIGHT * viewport.zoom;

      let drawX: number;
      let drawY: number;

      if (wall.edge === 'down') {
        drawX = bottomVertexX - scaledWidth;
        drawY = bottomVertexY - scaledHeight;
      } else {
        drawX = bottomVertexX;
        drawY = bottomVertexY - scaledHeight;
      }

      const srcX = r ? getAnimatedSourceX(r, objDef.frames, objDef.fps) : 0;
      this.ctx.drawImage(
        objDef.image,
        srcX,
        r ? r.y : 0,
        spriteW,
        spriteH,
        drawX,
        drawY,
        scaledWidth,
        scaledHeight
      );
    }
  }

  private renderInteriorEntities(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    for (const entity of interior.entities) {
      const worldCoord: WorldCoord = { wx: entity.x, wy: entity.y };
      const screen = worldToScreen(worldCoord, viewport);

      const size = 8 * viewport.zoom;
      this.ctx.fillStyle = '#ffd93d';
      this.ctx.strokeStyle = '#000000';
      this.ctx.lineWidth = 1;

      this.ctx.beginPath();
      this.ctx.arc(screen.sx, screen.sy + (TILE_HEIGHT / 2) * viewport.zoom, size, 0, Math.PI * 2);
      this.ctx.fill();
      this.ctx.stroke();

      if (viewport.zoom >= 0.5) {
        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `${10 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.fillText(
          entity.name || entity.entityId,
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom - size - 4
        );
      }
    }
  }

  private renderInteriorSpawnPoints(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    for (const spawn of interior.spawnPoints) {
      const worldCoord: WorldCoord = { wx: spawn.x, wy: spawn.y };
      const screen = worldToScreen(worldCoord, viewport);

      const hw = (TILE_WIDTH / 2) * viewport.zoom;
      const hh = (TILE_HEIGHT / 2) * viewport.zoom;

      this.ctx.fillStyle = 'rgba(0, 255, 100, 0.5)';
      this.ctx.beginPath();
      this.ctx.moveTo(screen.sx, screen.sy);
      this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
      this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
      this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
      this.ctx.closePath();
      this.ctx.fill();

      this.ctx.strokeStyle = 'rgba(0, 255, 100, 0.9)';
      this.ctx.lineWidth = 2;
      this.ctx.stroke();

      if (viewport.zoom >= 0.5) {
        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `bold ${10 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'middle';
        this.ctx.fillText(
          spawn.name,
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom
        );
      }
    }
  }

  private renderInteriorExitPortals(interior: InteriorMap, viewport: Viewport): void {
    if (!this.ctx) return;

    for (const exitPortal of interior.exitPortals) {
      for (let py = 0; py < exitPortal.height; py++) {
        for (let px = 0; px < exitPortal.width; px++) {
          const worldCoord: WorldCoord = { wx: exitPortal.x + px, wy: exitPortal.y + py };
          const screen = worldToScreen(worldCoord, viewport);

          const hw = (TILE_WIDTH / 2) * viewport.zoom;
          const hh = (TILE_HEIGHT / 2) * viewport.zoom;

          this.ctx.fillStyle = 'rgba(255, 165, 0, 0.5)';
          this.ctx.beginPath();
          this.ctx.moveTo(screen.sx, screen.sy);
          this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
          this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
          this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
          this.ctx.closePath();
          this.ctx.fill();

          this.ctx.strokeStyle = 'rgba(255, 165, 0, 0.9)';
          this.ctx.lineWidth = 2;
          this.ctx.stroke();
        }
      }

      if (viewport.zoom >= 0.5) {
        const worldCoord: WorldCoord = { wx: exitPortal.x, wy: exitPortal.y };
        const screen = worldToScreen(worldCoord, viewport);

        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `bold ${10 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'middle';
        this.ctx.fillText(
          'EXIT',
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom
        );
      }
    }
  }

  private isChunkVisible(chunk: Chunk, viewport: Viewport): boolean {
    if (!this.canvas) return true;

    const cx = chunk.coord.cx;
    const cy = chunk.coord.cy;

    const c0 = worldToScreen({ wx: cx * CHUNK_SIZE, wy: cy * CHUNK_SIZE }, viewport);
    const c1 = worldToScreen({ wx: (cx + 1) * CHUNK_SIZE, wy: cy * CHUNK_SIZE }, viewport);
    const c2 = worldToScreen({ wx: (cx + 1) * CHUNK_SIZE, wy: (cy + 1) * CHUNK_SIZE }, viewport);
    const c3 = worldToScreen({ wx: cx * CHUNK_SIZE, wy: (cy + 1) * CHUNK_SIZE }, viewport);

    const minSx = Math.min(c0.sx, c1.sx, c2.sx, c3.sx);
    const maxSx = Math.max(c0.sx, c1.sx, c2.sx, c3.sx);
    const minSy = Math.min(c0.sy, c1.sy, c2.sy, c3.sy);
    const maxSy = Math.max(c0.sy, c1.sy, c2.sy, c3.sy);

    // Generous vertical padding for tall objects/walls
    const TALL_SPRITE_PADDING = 500 * viewport.zoom;

    return maxSx >= 0 && minSx <= this.canvas.width &&
           maxSy >= 0 && (minSy - TALL_SPRITE_PADDING) <= this.canvas.height;
  }

  private renderChunkBounds(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    const corners: WorldCoord[] = [
      { wx: chunk.coord.cx * CHUNK_SIZE, wy: chunk.coord.cy * CHUNK_SIZE },
      { wx: (chunk.coord.cx + 1) * CHUNK_SIZE, wy: chunk.coord.cy * CHUNK_SIZE },
      { wx: (chunk.coord.cx + 1) * CHUNK_SIZE, wy: (chunk.coord.cy + 1) * CHUNK_SIZE },
      { wx: chunk.coord.cx * CHUNK_SIZE, wy: (chunk.coord.cy + 1) * CHUNK_SIZE },
    ];

    const screenCorners = corners.map((c) => worldToScreen(c, viewport));

    this.ctx.strokeStyle = chunk.dirty ? '#ff6b6b' : '#4ecdc4';
    this.ctx.lineWidth = 2;
    this.ctx.beginPath();
    this.ctx.moveTo(screenCorners[0].sx, screenCorners[0].sy);
    for (let i = 1; i < screenCorners.length; i++) {
      this.ctx.lineTo(screenCorners[i].sx, screenCorners[i].sy);
    }
    this.ctx.closePath();
    this.ctx.stroke();

    // Draw chunk label
    const centerWorld: WorldCoord = {
      wx: chunk.coord.cx * CHUNK_SIZE + CHUNK_SIZE / 2,
      wy: chunk.coord.cy * CHUNK_SIZE + CHUNK_SIZE / 2,
    };
    const centerScreen = worldToScreen(centerWorld, viewport);

    this.ctx.fillStyle = '#ffffff';
    this.ctx.font = `${12 * viewport.zoom}px monospace`;
    this.ctx.textAlign = 'center';
    this.ctx.textBaseline = 'middle';
    this.ctx.fillText(`${chunk.coord.cx},${chunk.coord.cy}`, centerScreen.sx, centerScreen.sy);
  }

  private renderCollisionOverlay(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    const bitset = new BitSet(chunk.width * chunk.height);
    bitset.setRaw(chunk.collision);

    this.ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';

    for (let i = 0; i < chunk.width * chunk.height; i++) {
      if (bitset.get(i)) {
        const local = indexToLocal(i, chunk.width);
        const worldCoord = chunkLocalToWorld(chunk.coord, local);
        const screen = worldToScreen(worldCoord, viewport);

        const hw = (TILE_WIDTH / 2) * viewport.zoom;
        const hh = (TILE_HEIGHT / 2) * viewport.zoom;

        this.ctx.beginPath();
        this.ctx.moveTo(screen.sx, screen.sy);
        this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
        this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
        this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
        this.ctx.closePath();
        this.ctx.fill();
      }
    }
  }

  private renderMapObjectsAndWalls(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    type Renderable =
      | { type: 'object'; obj: MapObject }
      | { type: 'wall'; wall: Wall };

    const renderables: Array<{ depth: number; item: Renderable }> = [];

    for (const obj of chunk.mapObjects) {
      renderables.push({
        depth: obj.x + obj.y,
        item: { type: 'object', obj }
      });
    }

    for (const wall of chunk.walls) {
      renderables.push({
        depth: wall.x + wall.y,
        item: { type: 'wall', wall }
      });
    }

    renderables.sort((a, b) => a.depth - b.depth);

    for (const { item } of renderables) {
      if (item.type === 'object') {
        this.renderMapObject(item.obj, chunk, viewport);
      } else {
        this.renderWall(item.wall, chunk, viewport);
      }
    }
  }

  private renderMapObject(obj: MapObject, chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    const worldCoord = chunkLocalToWorld(chunk.coord, { lx: obj.x, ly: obj.y });
    const screen = worldToScreen(worldCoord, viewport);

    const objDef = objectLoader.getObject(objectLoader.gidToId(obj.gid));

    if (objDef?.image) {
      const r = objDef.atlasRect;
      const spriteW = r ? r.w : obj.width;
      const spriteH = r ? r.h : obj.height;
      const scaledWidth = spriteW * viewport.zoom;
      const scaledHeight = spriteH * viewport.zoom;

      const drawX = screen.sx - scaledWidth / 2;
      const drawY = screen.sy + TILE_HEIGHT * viewport.zoom - scaledHeight;

      const srcX = r ? getAnimatedSourceX(r, objDef.frames, objDef.fps) : 0;
      this.ctx.drawImage(
        objDef.image,
        srcX,
        r ? r.y : 0,
        r ? r.w : objDef.image.width,
        r ? r.h : objDef.image.height,
        drawX,
        drawY,
        scaledWidth,
        scaledHeight
      );
    }
  }

  private renderWall(wall: Wall, chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    const worldCoord = chunkLocalToWorld(chunk.coord, { lx: wall.x, ly: wall.y });
    const screen = worldToScreen(worldCoord, viewport);

    const objDef = objectLoader.getWallByGid(wall.gid);

    if (objDef?.image) {
      const r = objDef.atlasRect;
      const spriteW = r ? r.w : objDef.image.width;
      const spriteH = r ? r.h : objDef.image.height;
      const scaledWidth = spriteW * viewport.zoom;
      const scaledHeight = spriteH * viewport.zoom;

      const bottomVertexX = screen.sx;
      const bottomVertexY = screen.sy + TILE_HEIGHT * viewport.zoom;

      let drawX: number;
      let drawY: number;

      if (wall.edge === 'down') {
        drawX = bottomVertexX - scaledWidth;
        drawY = bottomVertexY - scaledHeight;
      } else {
        drawX = bottomVertexX;
        drawY = bottomVertexY - scaledHeight;
      }

      const srcX = r ? getAnimatedSourceX(r, objDef.frames, objDef.fps) : 0;
      this.ctx.drawImage(
        objDef.image,
        srcX,
        r ? r.y : 0,
        spriteW,
        spriteH,
        drawX,
        drawY,
        scaledWidth,
        scaledHeight
      );
    }
  }

  private renderEntities(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx) return;

    for (const entity of chunk.entities) {
      const worldCoord = chunkLocalToWorld(chunk.coord, { lx: entity.x, ly: entity.y });
      const screen = worldToScreen(worldCoord, viewport);

      const size = 8 * viewport.zoom;
      this.ctx.fillStyle = '#ffd93d';
      this.ctx.strokeStyle = '#000000';
      this.ctx.lineWidth = 1;

      this.ctx.beginPath();
      this.ctx.arc(screen.sx, screen.sy + (TILE_HEIGHT / 2) * viewport.zoom, size, 0, Math.PI * 2);
      this.ctx.fill();
      this.ctx.stroke();

      if (viewport.zoom >= 0.5) {
        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `${10 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.fillText(
          entity.name || entity.entityId,
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom - size - 4
        );
      }
    }
  }

  private renderPortals(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx || !chunk.portals) return;

    for (const portal of chunk.portals) {
      for (let py = 0; py < portal.height; py++) {
        for (let px = 0; px < portal.width; px++) {
          const worldCoord = chunkLocalToWorld(chunk.coord, { lx: portal.x + px, ly: portal.y + py });
          const screen = worldToScreen(worldCoord, viewport);

          const hw = (TILE_WIDTH / 2) * viewport.zoom;
          const hh = (TILE_HEIGHT / 2) * viewport.zoom;

          this.ctx.fillStyle = 'rgba(128, 0, 255, 0.5)';
          this.ctx.beginPath();
          this.ctx.moveTo(screen.sx, screen.sy);
          this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
          this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
          this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
          this.ctx.closePath();
          this.ctx.fill();

          this.ctx.strokeStyle = 'rgba(180, 0, 255, 0.8)';
          this.ctx.lineWidth = 2;
          this.ctx.stroke();
        }
      }

      if (viewport.zoom >= 0.5) {
        const worldCoord = chunkLocalToWorld(chunk.coord, { lx: portal.x, ly: portal.y });
        const screen = worldToScreen(worldCoord, viewport);

        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `bold ${10 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'middle';

        const label = portal.targetMap || 'Portal';
        this.ctx.fillText(
          label,
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom
        );
      }
    }
  }

  private renderGatheringZones(chunk: Chunk, viewport: Viewport): void {
    if (!this.ctx || !chunk.gatheringZones || chunk.gatheringZones.length === 0) return;

    for (const gz of chunk.gatheringZones) {
      const worldCoord = chunkLocalToWorld(chunk.coord, { lx: gz.x, ly: gz.y });
      const screen = worldToScreen(worldCoord, viewport);

      const hw = (TILE_WIDTH / 2) * viewport.zoom;
      const hh = (TILE_HEIGHT / 2) * viewport.zoom;

      this.ctx.fillStyle = 'rgba(0, 180, 220, 0.4)';
      this.ctx.beginPath();
      this.ctx.moveTo(screen.sx, screen.sy);
      this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
      this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
      this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
      this.ctx.closePath();
      this.ctx.fill();

      this.ctx.strokeStyle = 'rgba(0, 220, 255, 0.8)';
      this.ctx.lineWidth = 2;
      this.ctx.stroke();

      if (viewport.zoom >= 0.5) {
        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `bold ${9 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'middle';
        this.ctx.fillText(
          gz.zoneId,
          screen.sx,
          screen.sy + (TILE_HEIGHT / 2) * viewport.zoom
        );
      }
    }
  }

  private renderNotes(viewport: Viewport): void {
    if (!this.ctx || !this.options.showNotes || this.notes.length === 0) return;

    const CATEGORY_COLORS: Record<string, string> = {
      todo: '#ff9800',
      bug: '#f44336',
      info: '#2196f3',
      idea: '#4caf50',
    };

    for (const note of this.notes) {
      const screen = worldToScreen({ wx: note.x, wy: note.y }, viewport);
      const isSelected = note.id === this.selectedNoteId;
      const isResolved = note.status === 'resolved';

      const hh = (TILE_HEIGHT / 2) * viewport.zoom;
      const centerX = screen.sx;
      const centerY = screen.sy + hh;

      this.ctx.save();
      if (isResolved) this.ctx.globalAlpha = 0.3;

      const radius = note.priority === 'high' ? 6 : 4;
      const color = CATEGORY_COLORS[note.category] || '#888';

      this.ctx.beginPath();
      this.ctx.arc(centerX, centerY, radius * viewport.zoom, 0, Math.PI * 2);
      this.ctx.fillStyle = color;
      this.ctx.fill();

      if (isSelected) {
        this.ctx.strokeStyle = '#fff';
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
      }

      if (viewport.zoom >= 0.5 && note.text) {
        const label = note.text.length > 25 ? note.text.slice(0, 22) + '...' : note.text;
        this.ctx.fillStyle = '#ffffff';
        this.ctx.font = `${9 * Math.max(1, viewport.zoom)}px sans-serif`;
        this.ctx.textAlign = 'center';
        this.ctx.textBaseline = 'bottom';
        this.ctx.fillText(label, centerX, centerY - radius * viewport.zoom - 2);
      }

      this.ctx.restore();
    }
  }

  private renderGrid(viewport: Viewport): void {
    if (!this.ctx || !this.canvas) return;

    this.ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
    this.ctx.lineWidth = 1;

    const topLeft = { wx: -50, wy: -50 };
    const bottomRight = { wx: 150, wy: 150 };

    for (let y = topLeft.wy; y <= bottomRight.wy; y++) {
      for (let x = topLeft.wx; x <= bottomRight.wx; x++) {
        const screen = worldToScreen({ wx: x, wy: y }, viewport);
        const hw = (TILE_WIDTH / 2) * viewport.zoom;
        const hh = (TILE_HEIGHT / 2) * viewport.zoom;

        this.ctx.beginPath();
        this.ctx.moveTo(screen.sx, screen.sy);
        this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
        this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
        this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
        this.ctx.closePath();
        this.ctx.stroke();
      }
    }
  }

  // Highlight a specific tile (for hover/selection)
  highlightTile(worldCoord: WorldCoord, viewport: Viewport, color: string = '#ffffff', fill: boolean = false): void {
    if (!this.ctx) return;

    const screen = worldToScreen(worldCoord, viewport);
    const hw = (TILE_WIDTH / 2) * viewport.zoom;
    const hh = (TILE_HEIGHT / 2) * viewport.zoom;

    this.ctx.beginPath();
    this.ctx.moveTo(screen.sx, screen.sy);
    this.ctx.lineTo(screen.sx + hw, screen.sy + hh);
    this.ctx.lineTo(screen.sx, screen.sy + TILE_HEIGHT * viewport.zoom);
    this.ctx.lineTo(screen.sx - hw, screen.sy + hh);
    this.ctx.closePath();

    if (fill) {
      this.ctx.fillStyle = color;
      this.ctx.fill();
    } else {
      this.ctx.strokeStyle = color;
      this.ctx.lineWidth = 2;
      this.ctx.stroke();
    }
  }

  // Draw selection rectangle
  renderSelection(start: WorldCoord, end: WorldCoord, viewport: Viewport): void {
    if (!this.ctx) return;

    const minX = Math.min(start.wx, end.wx);
    const maxX = Math.max(start.wx, end.wx);
    const minY = Math.min(start.wy, end.wy);
    const maxY = Math.max(start.wy, end.wy);

    this.ctx.strokeStyle = '#00ff00';
    this.ctx.lineWidth = 2;
    this.ctx.setLineDash([5, 5]);

    for (let y = minY; y <= maxY; y++) {
      for (let x = minX; x <= maxX; x++) {
        this.highlightTile({ wx: x, wy: y }, viewport, 'rgba(0, 255, 0, 0.3)');
      }
    }

    this.ctx.setLineDash([]);
  }
}

// Singleton instance
export const isometricRenderer = new IsometricRenderer();
