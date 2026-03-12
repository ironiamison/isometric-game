import type { ChunkCoord, LocalCoord, WorldCoord, ScreenCoord, Viewport } from '@/types';

export const CHUNK_SIZE = 32;
export const TILE_WIDTH = 64;
export const TILE_HEIGHT = 32;

// Convert world coordinates to chunk coordinates
export function worldToChunk(world: WorldCoord): ChunkCoord {
  return {
    cx: Math.floor(world.wx / CHUNK_SIZE),
    cy: Math.floor(world.wy / CHUNK_SIZE),
  };
}

// Convert world coordinates to local coordinates within a chunk
export function worldToLocal(world: WorldCoord): LocalCoord {
  return {
    lx: ((world.wx % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
    ly: ((world.wy % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
  };
}

// Convert chunk and local coordinates to world coordinates
export function chunkLocalToWorld(chunk: ChunkCoord, local: LocalCoord): WorldCoord {
  return {
    wx: chunk.cx * CHUNK_SIZE + local.lx,
    wy: chunk.cy * CHUNK_SIZE + local.ly,
  };
}

// Convert world coordinates to isometric screen coordinates
export function worldToScreen(world: WorldCoord, viewport: Viewport): ScreenCoord {
  const isoX = (world.wx - world.wy) * (TILE_WIDTH / 2);
  const isoY = (world.wx + world.wy) * (TILE_HEIGHT / 2);
  return {
    sx: isoX * viewport.zoom + viewport.offsetX,
    sy: isoY * viewport.zoom + viewport.offsetY,
  };
}

// Convert screen coordinates to world coordinates
export function screenToWorld(screen: ScreenCoord, viewport: Viewport): WorldCoord {
  // Remove viewport offset and zoom
  const isoX = (screen.sx - viewport.offsetX) / viewport.zoom;
  const isoY = (screen.sy - viewport.offsetY) / viewport.zoom;

  // Convert from isometric to world coordinates
  // isoX = (wx - wy) * (TILE_WIDTH / 2)
  // isoY = (wx + wy) * (TILE_HEIGHT / 2)
  // Solving for wx and wy:
  const wx = (isoX / (TILE_WIDTH / 2) + isoY / (TILE_HEIGHT / 2)) / 2;
  const wy = (isoY / (TILE_HEIGHT / 2) - isoX / (TILE_WIDTH / 2)) / 2;

  return { wx, wy };
}

// Convert screen coordinates to world tile (floored)
export function screenToWorldTile(screen: ScreenCoord, viewport: Viewport): WorldCoord {
  const world = screenToWorld(screen, viewport);
  return {
    wx: Math.floor(world.wx),
    wy: Math.floor(world.wy),
  };
}

// Get chunk key for Map storage
export function chunkKey(coord: ChunkCoord): string {
  return `${coord.cx},${coord.cy}`;
}

// Parse chunk key back to coordinates
export function parseChunkKey(key: string): ChunkCoord {
  const [cx, cy] = key.split(',').map(Number);
  return { cx, cy };
}

// Get local tile index within a chunk (row-major order)
export function localToIndex(local: LocalCoord, width: number = CHUNK_SIZE): number {
  return local.ly * width + local.lx;
}

// Get local coordinates from index
export function indexToLocal(index: number, width: number = CHUNK_SIZE): LocalCoord {
  return {
    lx: index % width,
    ly: Math.floor(index / width),
  };
}

// Check if world coordinate is within chunk bounds
export function isInChunk(world: WorldCoord, chunk: ChunkCoord): boolean {
  const minWx = chunk.cx * CHUNK_SIZE;
  const minWy = chunk.cy * CHUNK_SIZE;
  return (
    world.wx >= minWx &&
    world.wx < minWx + CHUNK_SIZE &&
    world.wy >= minWy &&
    world.wy < minWy + CHUNK_SIZE
  );
}

// Get all tiles in a diamond brush area.
// Odd sizes: diamond centered on tile (Manhattan distance radius).
// Even sizes: diamond centered on 2x2 block (top-left at center tile).
export function getBrushTiles(center: WorldCoord, brushSize: number): WorldCoord[] {
  if (brushSize <= 1) return [center];
  const tiles: WorldCoord[] = [];
  if (brushSize % 2 === 1) {
    // Odd: diamond with radius r centered on the tile
    const r = (brushSize - 1) / 2;
    for (let dy = -r; dy <= r; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        if (Math.abs(dx) + Math.abs(dy) <= r) {
          tiles.push({ wx: center.wx + dx, wy: center.wy + dy });
        }
      }
    }
  } else {
    // Even: 2x2 core expanded by diamond radius r
    const r = (brushSize - 2) / 2;
    for (let dy = -r; dy <= r + 1; dy++) {
      for (let dx = -r; dx <= r + 1; dx++) {
        // Distance from nearest point in the 2x2 core (0..1, 0..1)
        const distX = dx < 0 ? -dx : dx > 1 ? dx - 1 : 0;
        const distY = dy < 0 ? -dy : dy > 1 ? dy - 1 : 0;
        if (distX + distY <= r) {
          tiles.push({ wx: center.wx + dx, wy: center.wy + dy });
        }
      }
    }
  }
  return tiles;
}

// Get tile bounds in screen space for rendering
export function getTileScreenBounds(
  world: WorldCoord,
  viewport: Viewport
): { x: number; y: number; width: number; height: number } {
  const screen = worldToScreen(world, viewport);
  return {
    x: screen.sx - (TILE_WIDTH / 2) * viewport.zoom,
    y: screen.sy,
    width: TILE_WIDTH * viewport.zoom,
    height: TILE_HEIGHT * viewport.zoom,
  };
}
