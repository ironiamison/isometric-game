import { Router, type Request, type RequestParamHandler } from 'express';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

import { HttpError } from '../http.js';

export interface WorldDirectories {
  chunksDir: string;
  interiorsDir: string;
  gameChunksDir: string;
  gameInteriorsDir: string;
}

export interface MapRouterDependencies {
  getWorldDirs(world: string): WorldDirectories;
  getWorldFromRequest(req: Request): string;
  gameServerDir: string;
}

const COORDINATE_PATTERN = /^-?\d{1,7}$/;
const RESOURCE_ID_PATTERN = /^[a-zA-Z0-9_-]+$/;
const CHUNK_KEY_PATTERN = /^-?\d{1,7},-?\d{1,7}$/;
const CHUNK_FILE_PATTERN = /^-?\d{1,7}_-?\d{1,7}\.json$/;
const MAX_BULK_CHUNKS = 5_000;

interface ChunkPayload extends Record<string, unknown> {
  coord: { cx: number; cy: number };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isPositiveDimension(value: unknown): value is number {
  return Number.isInteger(value) && (value as number) > 0 && (value as number) <= 512;
}

function isTileLayer(value: unknown, expectedLength: number): value is number[] {
  return Array.isArray(value)
    && value.length === expectedLength
    && value.every((tile) => Number.isInteger(tile) && tile >= 0 && tile <= 0xffff_ffff);
}

function isPackedCollision(value: unknown, tileCount: number): value is number[] {
  return Array.isArray(value)
    && value.length === Math.ceil(tileCount / 8)
    && value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255);
}

function isValidChunk(value: unknown): value is ChunkPayload {
  if (!isRecord(value) || !isRecord(value.coord) || !isRecord(value.layers)) return false;
  const { width, height, coord, layers, collision } = value;
  if (!isPositiveDimension(width) || !isPositiveDimension(height)) return false;
  const tileCount = width * height;
  return Number.isInteger(coord.cx)
    && Number.isInteger(coord.cy)
    && Math.abs(coord.cx as number) <= 10_000_000
    && Math.abs(coord.cy as number) <= 10_000_000
    && isTileLayer(layers.ground, tileCount)
    && isTileLayer(layers.objects, tileCount)
    && isTileLayer(layers.overhead, tileCount)
    && isPackedCollision(collision, tileCount);
}

function isValidInterior(value: unknown): value is Record<string, unknown> {
  if (!isRecord(value) || !isRecord(value.size) || !isRecord(value.layers)) return false;
  if (!isPositiveDimension(value.size.width) || !isPositiveDimension(value.size.height)) {
    return false;
  }
  const tileCount = value.size.width * value.size.height;
  return typeof value.id === 'string'
    && RESOURCE_ID_PATTERN.test(value.id)
    && isTileLayer(value.layers.ground, tileCount)
    && isTileLayer(value.layers.objects, tileCount)
    && isTileLayer(value.layers.overhead, tileCount)
    && typeof value.collision === 'string';
}

async function writeJsonAtomically(filePath: string, value: unknown): Promise<void> {
  const tempPath = `${filePath}.${crypto.randomUUID()}.tmp`;
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  try {
    await fs.writeFile(tempPath, JSON.stringify(value, null, 2), 'utf-8');
    await fs.rename(tempPath, filePath);
  } catch (error) {
    await fs.rm(tempPath, { force: true });
    throw error;
  }
}

async function replaceJsonDirectoryAtomically(
  directory: string,
  entries: ReadonlyMap<string, unknown>,
): Promise<void> {
  const parent = path.dirname(directory);
  const basename = path.basename(directory);
  const operationId = crypto.randomUUID();
  const staging = path.join(parent, `.${basename}.${operationId}.staging`);
  const backup = path.join(parent, `.${basename}.${operationId}.backup`);
  await fs.mkdir(parent, { recursive: true });
  await fs.mkdir(staging);

  try {
    for (const [filename, value] of entries) {
      await writeJsonAtomically(path.join(staging, filename), value);
    }

    let hadExistingDirectory = false;
    try {
      await fs.rename(directory, backup);
      hadExistingDirectory = true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }

    try {
      await fs.rename(staging, directory);
    } catch (error) {
      if (hadExistingDirectory) {
        await fs.rename(backup, directory).catch(() => undefined);
      }
      throw error;
    }

    if (hadExistingDirectory) {
      await fs.rm(backup, { recursive: true, force: true });
    }
  } catch (error) {
    await fs.rm(staging, { recursive: true, force: true });
    throw error;
  }
}

export function createMapRouter(dependencies: MapRouterDependencies): Router {
  const router = Router();
  const {
    getWorldDirs,
    getWorldFromRequest,
    gameServerDir: GAME_SERVER_DIR,
  } = dependencies;

  const validateCoordinate: RequestParamHandler = (_req, res, next, value) => {
    if (!COORDINATE_PATTERN.test(value)) {
      return res.status(400).json({ error: 'Invalid chunk coordinate' });
    }
    return next();
  };
  router.param('cx', validateCoordinate);
  router.param('cy', validateCoordinate);
  router.param('id', (req, res, next, value) => {
    if (!RESOURCE_ID_PATTERN.test(value)) {
      return res.status(400).json({ error: 'Invalid interior id' });
    }
    return next();
  });
  router.use(
    ['/api/chunks', '/api/interiors', '/api/map', '/api/sync-from-game-server', '/api/deploy'],
    (req, res, next) => {
      try {
        getWorldFromRequest(req);
        return next();
      } catch (error) {
        if (error instanceof HttpError) {
          return res.status(error.status).json({ error: error.message });
        }
        return next(error);
      }
    },
  );

// --- Chunk API ---

// List all chunks
router.get('/api/chunks', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const files = await fs.readdir(chunksDir);
    const chunks = files
      .filter((file) => CHUNK_FILE_PATTERN.test(file))
      .map(f => {
        const [cx, cy] = f.replace('.json', '').split('_').map(Number);
        return { cx, cy };
      });
    res.json(chunks);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.json([]);
    } else {
      console.error('Error listing chunks:', err);
      res.status(500).json({ error: 'Failed to list chunks' });
    }
  }
});

// Get all chunks in one request
router.get('/api/chunks/all', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    console.log(`[GET /api/chunks/all] Loading from ${chunksDir}`);
    const files = await fs.readdir(chunksDir);
    const chunks: Record<string, unknown> = {};

    for (const file of files) {
      if (!CHUNK_FILE_PATTERN.test(file)) continue;
      const filePath = path.join(chunksDir, file);
      const data = await fs.readFile(filePath, 'utf-8');
      const key = file.replace('.json', '').replace('_', ',');
      chunks[key] = JSON.parse(data);
    }

    console.log(`  Loaded ${Object.keys(chunks).length} chunks`);
    // Prevent caching of chunk data
    res.set('Cache-Control', 'no-store, no-cache, must-revalidate');
    res.set('Pragma', 'no-cache');
    res.json(chunks);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      console.log(`  No chunks found (directory doesn't exist)`);
      res.json({});
    } else {
      console.error('Error loading all chunks:', err);
      res.status(500).json({ error: 'Failed to load chunks' });
    }
  }
});

// Get single chunk
router.get('/api/chunks/:cx/:cy', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { cx, cy } = req.params;
    const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
    const data = await fs.readFile(filePath, 'utf-8');
    res.json(JSON.parse(data));
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.status(404).json({ error: 'Chunk not found' });
    } else {
      console.error('Error reading chunk:', err);
      res.status(500).json({ error: 'Failed to read chunk' });
    }
  }
});

// Save single chunk
router.put('/api/chunks/:cx/:cy', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { cx, cy } = req.params;
    const chunk = req.body;
    if (!isValidChunk(chunk) || chunk.coord.cx !== Number(cx) || chunk.coord.cy !== Number(cy)) {
      return res.status(400).json({ error: 'Invalid chunk payload or coordinate mismatch' });
    }
    const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
    await writeJsonAtomically(filePath, chunk);
    res.json({ success: true });
  } catch (err) {
    console.error('Error saving chunk:', err);
    res.status(500).json({ error: 'Failed to save chunk' });
  }
});

// Save multiple chunks at once
router.put('/api/chunks', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    if (!isRecord(req.body)) {
      return res.status(400).json({ error: 'Chunk map must be an object' });
    }
    const chunks = req.body;
    if (Object.keys(chunks).length > MAX_BULK_CHUNKS) {
      return res.status(413).json({ error: `At most ${MAX_BULK_CHUNKS} chunks may be saved` });
    }
    for (const [key, chunk] of Object.entries(chunks)) {
      if (!CHUNK_KEY_PATTERN.test(key) || !isValidChunk(chunk)) {
        return res.status(400).json({ error: `Invalid chunk entry: ${key}` });
      }
      const [cx, cy] = key.split(',').map(Number);
      if (chunk.coord.cx !== cx || chunk.coord.cy !== cy) {
        return res.status(400).json({ error: `Chunk coordinate mismatch: ${key}` });
      }
    }
    console.log(`[PUT /api/chunks] Saving ${Object.keys(chunks).length} chunks to ${chunksDir}`);

    for (const [key, chunk] of Object.entries(chunks)) {
      const [cx, cy] = key.split(',');
      const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
      await writeJsonAtomically(filePath, chunk);
      console.log(`  Saved chunk ${key} to ${filePath}`);
    }

    res.json({ success: true, saved: Object.keys(chunks).length });
  } catch (err) {
    console.error('Error saving chunks:', err);
    res.status(500).json({ error: 'Failed to save chunks' });
  }
});

// Delete chunk
router.delete('/api/chunks/:cx/:cy', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { cx, cy } = req.params;
    const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
    await fs.unlink(filePath);
    res.json({ success: true });
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.json({ success: true }); // Already gone
    } else {
      console.error('Error deleting chunk:', err);
      res.status(500).json({ error: 'Failed to delete chunk' });
    }
  }
});

// --- Interior Maps API ---

// List all interior maps
router.get('/api/interiors', async (req, res) => {
  try {
    const { interiorsDir } = getWorldDirs(getWorldFromRequest(req));
    const files = await fs.readdir(interiorsDir);
    const interiors = files
      .filter(f => f.endsWith('.json'))
      .map(f => f.replace('.json', ''));
    res.json({ interiors });
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.json({ interiors: [] });
    } else {
      console.error('Error listing interiors:', err);
      res.status(500).json({ error: 'Failed to list interiors' });
    }
  }
});

// Get single interior map
router.get('/api/interiors/:id', async (req, res) => {
  try {
    const { interiorsDir } = getWorldDirs(getWorldFromRequest(req));
    const { id } = req.params;
    const filePath = path.join(interiorsDir, `${id}.json`);
    const data = await fs.readFile(filePath, 'utf-8');
    res.json(JSON.parse(data));
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.status(404).json({ error: 'Interior not found' });
    } else {
      console.error('Error reading interior:', err);
      res.status(500).json({ error: 'Failed to read interior' });
    }
  }
});

// Save interior map
router.put('/api/interiors/:id', async (req, res) => {
  try {
    const { interiorsDir } = getWorldDirs(getWorldFromRequest(req));
    const { id } = req.params;
    const interior = req.body;
    if (!isValidInterior(interior) || interior.id !== id) {
      return res.status(400).json({ error: 'Invalid interior payload or id mismatch' });
    }
    const filePath = path.join(interiorsDir, `${id}.json`);
    await writeJsonAtomically(filePath, interior);
    res.json({ success: true });
  } catch (err) {
    console.error('Error saving interior:', err);
    res.status(500).json({ error: 'Failed to save interior' });
  }
});

// Delete interior map
router.delete('/api/interiors/:id', async (req, res) => {
  try {
    const { interiorsDir } = getWorldDirs(getWorldFromRequest(req));
    const { id } = req.params;
    const filePath = path.join(interiorsDir, `${id}.json`);
    await fs.unlink(filePath);
    res.json({ success: true });
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      res.json({ success: true }); // Already gone
    } else {
      console.error('Error deleting interior:', err);
      res.status(500).json({ error: 'Failed to delete interior' });
    }
  }
});

// --- Map Export/Import ---

// Export entire map
router.get('/api/map/export', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const files = await fs.readdir(chunksDir);
    const chunks: Record<string, unknown> = {};

    for (const file of files) {
      if (!CHUNK_FILE_PATTERN.test(file)) continue;
      const filePath = path.join(chunksDir, file);
      const data = await fs.readFile(filePath, 'utf-8');
      const key = file.replace('.json', '').replace('_', ',');
      chunks[key] = JSON.parse(data);
    }

    res.setHeader('Content-Disposition', 'attachment; filename="map-export.json"');
    res.json({
      version: 1,
      exportedAt: new Date().toISOString(),
      chunks
    });
  } catch (err) {
    console.error('Error exporting map:', err);
    res.status(500).json({ error: 'Failed to export map' });
  }
});

// Import entire map
router.post('/api/map/import', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { chunks } = req.body;

    if (!isRecord(chunks)) {
      return res.status(400).json({ error: 'Invalid import format' });
    }
    if (Object.keys(chunks).length > MAX_BULK_CHUNKS) {
      return res.status(413).json({ error: `At most ${MAX_BULK_CHUNKS} chunks may be imported` });
    }
    const replacement = new Map<string, unknown>();
    for (const [key, chunk] of Object.entries(chunks)) {
      if (!CHUNK_KEY_PATTERN.test(key) || !isValidChunk(chunk)) {
        return res.status(400).json({ error: `Invalid chunk entry: ${key}` });
      }
      const [cx, cy] = key.split(',').map(Number);
      if (chunk.coord.cx !== cx || chunk.coord.cy !== cy) {
        return res.status(400).json({ error: `Chunk coordinate mismatch: ${key}` });
      }
      replacement.set(`${cx}_${cy}.json`, chunk);
    }

    await replaceJsonDirectoryAtomically(chunksDir, replacement);
    res.json({ success: true, imported: replacement.size });
  } catch (err) {
    console.error('Error importing map:', err);
    res.status(500).json({ error: 'Failed to import map' });
  }
});

// --- Sync from Game Server ---

// Convert base64-encoded collision bitset to array
function base64ToCollision(base64: string): number[] {
  const binary = Buffer.from(base64, 'base64').toString('binary');
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return Array.from(bytes);
}

// Convert game server chunk format to mapper format
function convertChunkToMapperFormat(gameChunk: Record<string, unknown>): Record<string, unknown> {
  const size = gameChunk.size as number || 32;

  // Convert collision from base64 to array
  let collisionArray: number[] = [];
  if (typeof gameChunk.collision === 'string') {
    collisionArray = base64ToCollision(gameChunk.collision);
  } else if (Array.isArray(gameChunk.collision)) {
    collisionArray = gameChunk.collision;
  }

  // Preserve elevation data so sync round-trips raised tiles / cliffs.
  const heightmap = (gameChunk.heightmap ?? gameChunk.heights) as number[] | undefined;
  const blockTypesDown = gameChunk.blockTypesDown as number[] | undefined;
  const blockTypesRight = gameChunk.blockTypesRight as number[] | undefined;

  return {
    coord: gameChunk.coord,
    width: size,
    height: size,
    layers: gameChunk.layers,
    collision: collisionArray,
    entities: gameChunk.entities || [],
    mapObjects: gameChunk.mapObjects || [],
    walls: gameChunk.walls || [],
    portals: gameChunk.portals || [],
    gatheringZones: gameChunk.gatheringZones || [],
    farmingPlots: gameChunk.farmingPlots || [],
    ...(Array.isArray(heightmap) && heightmap.length > 0 ? { heightmap } : {}),
    ...(Array.isArray(blockTypesDown) && blockTypesDown.length > 0 ? { blockTypesDown } : {}),
    ...(Array.isArray(blockTypesRight) && blockTypesRight.length > 0 ? { blockTypesRight } : {}),
  };
}

// Sync maps FROM game server TO mapper (reverse of deploy)
router.post('/api/sync-from-game-server', async (req, res) => {
  try {
    const world = getWorldFromRequest(req);
    const { chunksDir, interiorsDir, gameChunksDir, gameInteriorsDir } = getWorldDirs(world);

    // Ensure mapper data directories exist
    await fs.mkdir(chunksDir, { recursive: true });
    await fs.mkdir(interiorsDir, { recursive: true });

    let chunksSynced = 0;
    let interiorsSynced = 0;

    // Convert and copy chunks (game server format -> mapper format)
    try {
      const chunkFiles = await fs.readdir(gameChunksDir);
      for (const file of chunkFiles) {
        if (!file.endsWith('.json') || !file.startsWith('chunk_')) continue;
        const srcPath = path.join(gameChunksDir, file);

        // Read and convert chunk
        const data = await fs.readFile(srcPath, 'utf-8');
        const gameChunk = JSON.parse(data);
        const mapperChunk = convertChunkToMapperFormat(gameChunk);

        // Write to mapper without chunk_ prefix
        const destFilename = file.replace('chunk_', '');
        const destPath = path.join(chunksDir, destFilename);
        await fs.writeFile(destPath, JSON.stringify(mapperChunk, null, 2));
        chunksSynced++;
      }
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
        throw err;
      }
    }

    // Copy interiors
    try {
      const interiorFiles = await fs.readdir(gameInteriorsDir);
      for (const file of interiorFiles) {
        if (!file.endsWith('.json')) continue;
        const srcPath = path.join(gameInteriorsDir, file);
        const destPath = path.join(interiorsDir, file);
        await fs.copyFile(srcPath, destPath);
        interiorsSynced++;
      }
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
        throw err;
      }
    }

    console.log(`Synced ${chunksSynced} chunks and ${interiorsSynced} interiors from game server (${world})`);
    res.json({
      success: true,
      chunksSynced,
      interiorsSynced,
      source: GAME_SERVER_DIR
    });
  } catch (err) {
    console.error('Sync from game server failed:', err);
    res.status(500).json({ error: `Sync failed: ${(err as Error).message}` });
  }
});

// --- Deploy to Game Server ---

// Convert collision array to base64-encoded bitset
function collisionToBase64(collision: number[], size: number): string {
  const byteLength = Math.ceil(size / 8);
  const bytes = new Uint8Array(byteLength);

  for (let i = 0; i < collision.length && i < byteLength; i++) {
    bytes[i] = collision[i];
  }

  // Convert to base64
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return Buffer.from(binary, 'binary').toString('base64');
}

// Convert mapper chunk format to game server format
function convertChunkToGameFormat(mapperChunk: Record<string, unknown>): Record<string, unknown> {
  const width = mapperChunk.width as number;
  const size = width; // Assuming square chunks

  // Convert collision array to base64
  const collisionArray = mapperChunk.collision as number[] || [];
  const collisionBase64 = collisionToBase64(collisionArray, size * size);

  // Height/elevation data (raised tiles / cliffs). The game loader only
  // builds height data when `heightmap` is present, so emit these only when
  // the chunk actually has elevation — otherwise it stays flat as before.
  const heightmap = (mapperChunk.heightmap ?? mapperChunk.heights) as number[] | undefined;
  const blockTypesDown = mapperChunk.blockTypesDown as number[] | undefined;
  const blockTypesRight = mapperChunk.blockTypesRight as number[] | undefined;

  return {
    version: 2,
    coord: mapperChunk.coord,
    size,
    layers: mapperChunk.layers,
    collision: collisionBase64,
    entities: mapperChunk.entities || [],
    mapObjects: mapperChunk.mapObjects || [],
    walls: mapperChunk.walls || [],
    portals: mapperChunk.portals || [],
    gatheringZones: mapperChunk.gatheringZones || [],
    farmingPlots: mapperChunk.farmingPlots || [],
    ...(Array.isArray(heightmap) && heightmap.length > 0 ? { heightmap } : {}),
    ...(Array.isArray(blockTypesDown) && blockTypesDown.length > 0 ? { blockTypesDown } : {}),
    ...(Array.isArray(blockTypesRight) && blockTypesRight.length > 0 ? { blockTypesRight } : {}),
  };
}

// Deploy maps to game server directory
router.post('/api/deploy', async (req, res) => {
  try {
    const world = getWorldFromRequest(req);
    const { chunksDir, interiorsDir, gameChunksDir, gameInteriorsDir } = getWorldDirs(world);

    // Ensure game server directories exist
    await fs.mkdir(gameChunksDir, { recursive: true });
    await fs.mkdir(gameInteriorsDir, { recursive: true });

    let chunksCopied = 0;
    let interiorsCopied = 0;

    // Convert and copy chunks (mapper format -> game server format)
    try {
      const chunkFiles = await fs.readdir(chunksDir);
      for (const file of chunkFiles) {
        if (!file.endsWith('.json')) continue;
        const srcPath = path.join(chunksDir, file);

        // Read and convert chunk
        const data = await fs.readFile(srcPath, 'utf-8');
        const mapperChunk = JSON.parse(data);
        const gameChunk = convertChunkToGameFormat(mapperChunk);

        // Write to game server with chunk_ prefix
        const destFilename = `chunk_${file}`;
        const destPath = path.join(gameChunksDir, destFilename);
        await fs.writeFile(destPath, JSON.stringify(gameChunk, null, 2));
        chunksCopied++;
      }
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
        throw err;
      }
    }

    // Copy interiors (same filename format - already in correct format)
    try {
      const interiorFiles = await fs.readdir(interiorsDir);
      for (const file of interiorFiles) {
        if (!file.endsWith('.json')) continue;
        const srcPath = path.join(interiorsDir, file);
        const destPath = path.join(gameInteriorsDir, file);
        await fs.copyFile(srcPath, destPath);
        interiorsCopied++;
      }
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
        throw err;
      }
    }

    console.log(`Deployed ${chunksCopied} chunks and ${interiorsCopied} interiors to game server (${world})`);

    // Optionally restart the game server so the deployed maps go live.
    // Gated on GAME_SERVER_SERVICE so this is a no-op unless an environment
    // (e.g. staging) explicitly opts in by naming a systemd unit to restart.
    let restarted = false;
    let restartError: string | undefined;
    const gameService = process.env.GAME_SERVER_SERVICE;
    if (req.body?.restart === true) {
      if (!gameService) {
        restartError = 'GAME_SERVER_SERVICE is not configured; skipped restart';
        console.warn(restartError);
      } else {
        try {
          await execFileAsync('systemctl', ['restart', gameService], { timeout: 30000 });
          restarted = true;
          console.log(`Restarted game server service: ${gameService}`);
        } catch (err) {
          restartError = (err as Error).message;
          console.error(`Failed to restart ${gameService}:`, err);
        }
      }
    }

    res.json({
      success: true,
      chunksCopied,
      interiorsCopied,
      destination: GAME_SERVER_DIR,
      restarted,
      service: gameService,
      ...(restartError ? { restartError } : {}),
    });
  } catch (err) {
    console.error('Deploy failed:', err);
    res.status(500).json({ error: `Deploy failed: ${(err as Error).message}` });
  }
});


  return router;
}
