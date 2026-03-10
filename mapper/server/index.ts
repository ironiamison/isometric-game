import express from 'express';
import cors from 'cors';
import crypto from 'crypto';
import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';
import { exec } from 'child_process';
import { promisify } from 'util';
import multer from 'multer';

const execAsync = promisify(exec);

// Read PNG dimensions from file header (bytes 16-23 of a valid PNG)
async function readPngDimensions(filePath: string): Promise<{ width: number; height: number }> {
  const fd = await fs.open(filePath, 'r');
  try {
    const buf = Buffer.alloc(24);
    await fd.read(buf, 0, 24, 0);
    // Validate PNG signature
    if (buf[0] !== 0x89 || buf[1] !== 0x50 || buf[2] !== 0x4E || buf[3] !== 0x47) {
      throw new Error('Not a valid PNG file');
    }
    // Width and height are at bytes 16-19 and 20-23 (big-endian uint32)
    const width = buf.readUInt32BE(16);
    const height = buf.readUInt32BE(20);
    return { width, height };
  } finally {
    await fd.close();
  }
}

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const app = express();
const PORT = process.env.PORT || 3000;

// Detect if running from compiled dist or source
// In dev: __dirname = /path/to/mapper/server
// In prod: __dirname = /path/to/mapper/server/dist
const isCompiled = __dirname.endsWith('dist');
const serverRoot = isCompiled ? path.join(__dirname, '..') : __dirname;
const mapperRoot = path.join(serverRoot, '..');
const projectRoot = path.join(mapperRoot, '..');

// Data directory for chunk storage
const DATA_DIR = path.join(mapperRoot, 'mapper-data');
const NOTES_FILE = path.join(DATA_DIR, 'notes.json');

// Valid worlds
const VALID_WORLDS = ['world_0', 'world_1'] as const;

function getWorldDirs(world: string) {
  return {
    chunksDir: path.join(DATA_DIR, world, 'chunks'),
    interiorsDir: path.join(DATA_DIR, world, 'interiors'),
    gameChunksDir: path.join(GAME_SERVER_DIR, world),
    gameInteriorsDir: path.join(GAME_SERVER_DIR, 'interiors'), // shared
  };
}

function getWorldFromRequest(req: express.Request): string {
  const world = (req.query.world as string) || 'world_0';
  if (!VALID_WORLDS.includes(world as any)) throw new Error(`Invalid world: ${world}`);
  return world;
}

// Asset directories
const CLIENT_ASSETS_DIR = path.join(projectRoot, 'client', 'assets');
const CLIENT_SPRITES_DIR = path.join(CLIENT_ASSETS_DIR, 'sprites');
const MAPPER_PUBLIC_ASSETS = path.join(mapperRoot, 'public', 'assets');
const MAPPER_SPRITES_DIR = path.join(MAPPER_PUBLIC_ASSETS, 'sprites');
const MAPPER_CONFIG_PATH = path.join(mapperRoot, 'mapper-config.json');
const TILES_EXTRACTED_DIR = path.join(CLIENT_SPRITES_DIR, 'tiles_extracted');

// Game server maps directory (for deploy)
const GAME_SERVER_DIR = path.join(mapperRoot, '..', 'rust-server', 'maps');

// Multer storage for file uploads (temp location)
const upload = multer({ dest: path.join(DATA_DIR, 'uploads') });

// Ensure data directories exist (with migration from flat layout to per-world)
async function ensureDataDirs() {
  await fs.mkdir(path.join(DATA_DIR, 'uploads'), { recursive: true });

  // Migration: if old flat layout exists (mapper-data/chunks/) but new layout doesn't, move data
  const oldChunksDir = path.join(DATA_DIR, 'chunks');
  const oldInteriorsDir = path.join(DATA_DIR, 'interiors');
  const newWorld0Chunks = path.join(DATA_DIR, 'world_0', 'chunks');

  try {
    await fs.access(oldChunksDir);
    try {
      await fs.access(newWorld0Chunks);
    } catch {
      // Old layout exists, new doesn't — migrate
      console.log('Migrating mapper-data from flat layout to per-world layout...');
      await fs.mkdir(path.join(DATA_DIR, 'world_0'), { recursive: true });
      await fs.rename(oldChunksDir, newWorld0Chunks);
      console.log('  Moved chunks/ -> world_0/chunks/');
      try {
        await fs.access(oldInteriorsDir);
        await fs.rename(oldInteriorsDir, path.join(DATA_DIR, 'world_0', 'interiors'));
        console.log('  Moved interiors/ -> world_0/interiors/');
      } catch {
        // No old interiors dir, that's fine
      }
    }
  } catch {
    // No old chunks dir, no migration needed
  }

  // Ensure all world directories exist
  for (const world of VALID_WORLDS) {
    const dirs = getWorldDirs(world);
    await fs.mkdir(dirs.chunksDir, { recursive: true });
    await fs.mkdir(dirs.interiorsDir, { recursive: true });
  }
}

// In-memory notes cache
let notesCache: any[] = [];

async function loadNotesFromDisk(): Promise<any[]> {
  try {
    const data = await fs.readFile(NOTES_FILE, 'utf-8');
    return JSON.parse(data);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw err;
  }
}

async function saveNotesToDisk(notes: any[]): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
  await fs.writeFile(NOTES_FILE, JSON.stringify(notes, null, 2));
}

// Middleware
app.use(cors());
app.use(express.json({ limit: '50mb' }));
app.use(express.urlencoded({ extended: false }));

// --- Auth ---
const AUTH_USER = process.env.MAPPER_USER || 'null';
const AUTH_PASS = process.env.MAPPER_PASS || 'NANULL!';
const AUTH_SECRET = crypto.randomBytes(32).toString('hex');

function makeToken(): string {
  return crypto.createHmac('sha256', AUTH_SECRET).update(AUTH_USER + AUTH_PASS).digest('hex');
}

function parseCookies(header: string | undefined): Record<string, string> {
  const cookies: Record<string, string> = {};
  if (!header) return cookies;
  for (const part of header.split(';')) {
    const [k, ...v] = part.trim().split('=');
    if (k) cookies[k] = v.join('=');
  }
  return cookies;
}

const LOGIN_HTML = `<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Mapper Login</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500&family=Outfit:wght@600&display=swap" rel="stylesheet">
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: 'DM Sans', sans-serif;
    min-height: 100vh;
    display: flex;
    justify-content: center;
    align-items: center;
    background: #0c0e1a;
    color: #c8cad0;
    overflow: hidden;
  }

  /* isometric grid background */
  body::before {
    content: '';
    position: fixed;
    inset: 0;
    background-image:
      linear-gradient(30deg, rgba(78, 205, 196, 0.03) 1px, transparent 1px),
      linear-gradient(150deg, rgba(78, 205, 196, 0.03) 1px, transparent 1px),
      linear-gradient(-30deg, rgba(78, 205, 196, 0.03) 1px, transparent 1px),
      linear-gradient(-150deg, rgba(78, 205, 196, 0.03) 1px, transparent 1px);
    background-size: 60px 34px;
    background-position: 0 0, 0 0, 30px 17px, 30px 17px;
    z-index: 0;
  }

  /* soft radial glow */
  body::after {
    content: '';
    position: fixed;
    top: 40%;
    left: 50%;
    width: 600px;
    height: 600px;
    transform: translate(-50%, -50%);
    background: radial-gradient(circle, rgba(78, 205, 196, 0.06) 0%, transparent 70%);
    z-index: 0;
    pointer-events: none;
  }

  .login-container {
    position: relative;
    z-index: 1;
    width: 100%;
    max-width: 360px;
    padding: 0 20px;
    animation: fadeUp 0.5s ease-out;
  }

  @keyframes fadeUp {
    from { opacity: 0; transform: translateY(16px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .brand {
    text-align: center;
    margin-bottom: 32px;
  }

  .brand-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border-radius: 12px;
    background: linear-gradient(135deg, #4ecdc4 0%, #3a9e97 100%);
    margin-bottom: 16px;
    box-shadow: 0 4px 24px rgba(78, 205, 196, 0.2);
  }

  .brand-icon svg {
    width: 24px;
    height: 24px;
    fill: none;
    stroke: #0c0e1a;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .brand h1 {
    font-family: 'Outfit', sans-serif;
    font-size: 22px;
    font-weight: 600;
    color: #eef0f4;
    letter-spacing: -0.02em;
  }

  .brand p {
    font-size: 13px;
    color: #6b6e7a;
    margin-top: 6px;
  }

  .card {
    background: rgba(22, 25, 40, 0.7);
    border: 1px solid rgba(78, 205, 196, 0.08);
    border-radius: 16px;
    padding: 32px 28px;
    backdrop-filter: blur(12px);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.03) inset,
      0 16px 48px rgba(0, 0, 0, 0.3);
  }

  .field {
    margin-bottom: 20px;
  }

  .field label {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: #8a8d9a;
    margin-bottom: 8px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .field input {
    display: block;
    width: 100%;
    padding: 12px 14px;
    background: rgba(12, 14, 26, 0.6);
    border: 1px solid rgba(78, 205, 196, 0.1);
    border-radius: 10px;
    color: #eef0f4;
    font-family: 'DM Sans', sans-serif;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .field input:focus {
    border-color: rgba(78, 205, 196, 0.35);
    box-shadow: 0 0 0 3px rgba(78, 205, 196, 0.08);
  }

  .field input::placeholder {
    color: #3d3f4e;
  }

  button[type="submit"] {
    display: block;
    width: 100%;
    padding: 12px;
    margin-top: 24px;
    background: linear-gradient(135deg, #4ecdc4 0%, #3dbdb5 100%);
    color: #0c0e1a;
    font-family: 'DM Sans', sans-serif;
    font-size: 14px;
    font-weight: 500;
    border: none;
    border-radius: 10px;
    cursor: pointer;
    letter-spacing: 0.01em;
    transition: transform 0.15s, box-shadow 0.2s, filter 0.2s;
    box-shadow: 0 2px 12px rgba(78, 205, 196, 0.2);
  }

  button[type="submit"]:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 20px rgba(78, 205, 196, 0.3);
    filter: brightness(1.05);
  }

  button[type="submit"]:active {
    transform: translateY(0);
    box-shadow: 0 1px 6px rgba(78, 205, 196, 0.15);
  }

  .error {
    margin-top: 16px;
    padding: 10px 14px;
    background: rgba(233, 69, 96, 0.08);
    border: 1px solid rgba(233, 69, 96, 0.2);
    border-radius: 8px;
    color: #f07088;
    font-size: 13px;
    text-align: center;
  }
</style></head><body>
<div class="login-container">
  <div class="brand">
    <div class="brand-icon">
      <svg viewBox="0 0 24 24"><path d="M12 3L2 9l10 6 10-6-10-6z"/><path d="M2 15l10 6 10-6"/><path d="M2 9v6"/><path d="M22 9v6"/></svg>
    </div>
    <h1>Mapper</h1>
    <p>World editor</p>
  </div>
  <div class="card">
    <form method="POST" action="/mapper/login">
      <div class="field">
        <label for="username">Username</label>
        <input id="username" name="username" required autocomplete="username" placeholder="Enter username">
      </div>
      <div class="field">
        <label for="password">Password</label>
        <input id="password" name="password" type="password" required autocomplete="current-password" placeholder="Enter password">
      </div>
      <button type="submit">Sign in</button>
      ERRPLACEHOLDER
    </form>
  </div>
</div>
</body></html>`;

app.post('/login', (req, res) => {
  const { username, password } = req.body || {};
  if (username === AUTH_USER && password === AUTH_PASS) {
    res.setHeader('Set-Cookie', `mapper_token=${makeToken()}; Path=/mapper; HttpOnly; SameSite=Lax; Max-Age=31536000`);
    return res.redirect('/mapper/');
  }
  res.status(401).send(LOGIN_HTML.replace('ERRPLACEHOLDER', '<p class="error">Invalid credentials</p>'));
});

app.get('/login', (_req, res) => {
  res.send(LOGIN_HTML.replace('ERRPLACEHOLDER', ''));
});

// Auth check for all routes (except login)
app.use((req, res, next) => {
  if (req.path === '/login') return next();
  const cookies = parseCookies(req.headers.cookie);
  if (cookies.mapper_token === makeToken()) return next();
  return res.redirect('/mapper/login');
});

// Frontend dist path (served after API routes)
const distPath = path.join(mapperRoot, 'dist');

// --- User Info ---
app.get('/api/me', (_req, res) => {
  const worlds: string[] = ['world_0'];
  if (AUTH_USER === 'null') {
    worlds.push('world_1');
  }
  res.json({ username: AUTH_USER, worlds });
});

// Serve mapper-config.json from root
app.get('/mapper-config.json', (_req, res) => {
  res.sendFile(path.join(mapperRoot, 'mapper-config.json'));
});

// --- Chunk API ---

// List all chunks
app.get('/api/chunks', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const files = await fs.readdir(chunksDir);
    const chunks = files
      .filter(f => f.endsWith('.json'))
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
app.get('/api/chunks/all', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    console.log(`[GET /api/chunks/all] Loading from ${chunksDir}`);
    const files = await fs.readdir(chunksDir);
    const chunks: Record<string, unknown> = {};

    for (const file of files) {
      if (!file.endsWith('.json')) continue;
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
app.get('/api/chunks/:cx/:cy', async (req, res) => {
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
app.put('/api/chunks/:cx/:cy', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { cx, cy } = req.params;
    const chunk = req.body;
    const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
    await fs.writeFile(filePath, JSON.stringify(chunk, null, 2));
    res.json({ success: true });
  } catch (err) {
    console.error('Error saving chunk:', err);
    res.status(500).json({ error: 'Failed to save chunk' });
  }
});

// Save multiple chunks at once
app.put('/api/chunks', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const chunks = req.body as Record<string, unknown>;
    console.log(`[PUT /api/chunks] Saving ${Object.keys(chunks).length} chunks to ${chunksDir}`);

    for (const [key, chunk] of Object.entries(chunks)) {
      const [cx, cy] = key.split(',');
      const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
      await fs.writeFile(filePath, JSON.stringify(chunk, null, 2));
      console.log(`  Saved chunk ${key} to ${filePath}`);
    }

    res.json({ success: true, saved: Object.keys(chunks).length });
  } catch (err) {
    console.error('Error saving chunks:', err);
    res.status(500).json({ error: 'Failed to save chunks' });
  }
});

// Delete chunk
app.delete('/api/chunks/:cx/:cy', async (req, res) => {
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
app.get('/api/interiors', async (req, res) => {
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
app.get('/api/interiors/:id', async (req, res) => {
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
app.put('/api/interiors/:id', async (req, res) => {
  try {
    const { interiorsDir } = getWorldDirs(getWorldFromRequest(req));
    const { id } = req.params;
    const interior = req.body;
    const filePath = path.join(interiorsDir, `${id}.json`);
    await fs.writeFile(filePath, JSON.stringify(interior, null, 2));
    res.json({ success: true });
  } catch (err) {
    console.error('Error saving interior:', err);
    res.status(500).json({ error: 'Failed to save interior' });
  }
});

// Delete interior map
app.delete('/api/interiors/:id', async (req, res) => {
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
app.get('/api/map/export', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const files = await fs.readdir(chunksDir);
    const chunks: Record<string, unknown> = {};

    for (const file of files) {
      if (!file.endsWith('.json')) continue;
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
app.post('/api/map/import', async (req, res) => {
  try {
    const { chunksDir } = getWorldDirs(getWorldFromRequest(req));
    const { chunks } = req.body;

    if (!chunks || typeof chunks !== 'object') {
      return res.status(400).json({ error: 'Invalid import format' });
    }

    // Clear existing chunks
    try {
      const existingFiles = await fs.readdir(chunksDir);
      for (const file of existingFiles) {
        await fs.unlink(path.join(chunksDir, file));
      }
    } catch {
      // Directory might not exist yet
    }

    // Write new chunks
    let count = 0;
    for (const [key, chunk] of Object.entries(chunks)) {
      const [cx, cy] = key.split(',');
      const filePath = path.join(chunksDir, `${cx}_${cy}.json`);
      await fs.writeFile(filePath, JSON.stringify(chunk, null, 2));
      count++;
    }

    res.json({ success: true, imported: count });
  } catch (err) {
    console.error('Error importing map:', err);
    res.status(500).json({ error: 'Failed to import map' });
  }
});

// --- Sync from Game Server ---

// Convert base64-encoded collision bitset to array
function base64ToCollision(base64: string, size: number): number[] {
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
    collisionArray = base64ToCollision(gameChunk.collision, size * size);
  } else if (Array.isArray(gameChunk.collision)) {
    collisionArray = gameChunk.collision;
  }

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
  };
}

// Sync maps FROM game server TO mapper (reverse of deploy)
app.post('/api/sync-from-game-server', async (req, res) => {
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
  const height = mapperChunk.height as number;
  const size = width; // Assuming square chunks

  // Convert collision array to base64
  const collisionArray = mapperChunk.collision as number[] || [];
  const collisionBase64 = collisionToBase64(collisionArray, size * size);

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
  };
}

// Deploy maps to game server directory
app.post('/api/deploy', async (req, res) => {
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
    res.json({
      success: true,
      chunksCopied,
      interiorsCopied,
      destination: GAME_SERVER_DIR
    });
  } catch (err) {
    console.error('Deploy failed:', err);
    res.status(500).json({ error: `Deploy failed: ${(err as Error).message}` });
  }
});

// --- Notes API ---

// Get all notes (with optional filters)
app.get('/api/notes', (_req, res) => {
  let filtered = notesCache;

  const { status, category, priority, chunk } = _req.query;
  if (status) filtered = filtered.filter(n => n.status === status);
  if (category) filtered = filtered.filter(n => n.category === category);
  if (priority) filtered = filtered.filter(n => n.priority === priority);
  if (chunk) {
    const [cx, cy] = (chunk as string).split(',').map(Number);
    filtered = filtered.filter(n => n.chunkCoord?.cx === cx && n.chunkCoord?.cy === cy);
  }

  res.json(filtered);
});

// Get single note
app.get('/api/notes/:id', (req, res) => {
  const note = notesCache.find(n => n.id === req.params.id);
  if (!note) return res.status(404).json({ error: 'Note not found' });
  res.json(note);
});

// Create note
app.post('/api/notes', async (req, res) => {
  try {
    const note = req.body;
    if (!note.id || !note.text === undefined) {
      return res.status(400).json({ error: 'Note must have id and text' });
    }
    notesCache.push(note);
    await saveNotesToDisk(notesCache);
    res.json(note);
  } catch (err) {
    console.error('Error creating note:', err);
    res.status(500).json({ error: 'Failed to create note' });
  }
});

// Update note
app.put('/api/notes/:id', async (req, res) => {
  try {
    const idx = notesCache.findIndex(n => n.id === req.params.id);
    if (idx === -1) return res.status(404).json({ error: 'Note not found' });
    notesCache[idx] = { ...notesCache[idx], ...req.body, id: req.params.id };
    await saveNotesToDisk(notesCache);
    res.json(notesCache[idx]);
  } catch (err) {
    console.error('Error updating note:', err);
    res.status(500).json({ error: 'Failed to update note' });
  }
});

// Delete note
app.delete('/api/notes/:id', async (req, res) => {
  try {
    const idx = notesCache.findIndex(n => n.id === req.params.id);
    if (idx === -1) return res.json({ success: true }); // idempotent
    notesCache.splice(idx, 1);
    await saveNotesToDisk(notesCache);
    res.json({ success: true });
  } catch (err) {
    console.error('Error deleting note:', err);
    res.status(500).json({ error: 'Failed to delete note' });
  }
});

// --- Asset Import API ---

interface MapperConfig {
  tilesets: Array<{ name: string; image: string; tileWidth: number; tileHeight: number; columns: number; firstGid: number }>;
  objects?: { basePath: string; firstGid: number; items: Array<{ id: number; name: string; width: number; height: number }> };
  walls?: { basePath: string; firstGid: number; items: Array<{ id: number; name: string; width: number; height: number }> };
  chunkSize: number;
  mapsPath: string;
  entitiesPath: string;
}

async function readMapperConfig(): Promise<MapperConfig> {
  const data = await fs.readFile(MAPPER_CONFIG_PATH, 'utf-8');
  return JSON.parse(data);
}

async function writeMapperConfig(config: MapperConfig): Promise<void> {
  const tmpPath = MAPPER_CONFIG_PATH + '.tmp';
  await fs.writeFile(tmpPath, JSON.stringify(config, null, 2));
  await fs.rename(tmpPath, MAPPER_CONFIG_PATH);
}

async function getNextId(category: string): Promise<number> {
  const dir = path.join(CLIENT_SPRITES_DIR, category);
  try {
    const files = await fs.readdir(dir);
    let maxId = 0;
    for (const f of files) {
      const match = f.match(/^(\d+)\.png$/);
      if (match) {
        maxId = Math.max(maxId, parseInt(match[1]));
      }
    }
    return maxId + 1;
  } catch {
    return 1;
  }
}

async function copyAssetToMapper(): Promise<void> {
  // Copy rebuilt atlases and manifest from client/assets to mapper/public/assets
  const filesToCopy = [
    { src: path.join(CLIENT_ASSETS_DIR, 'sprite_manifest.json'), dest: path.join(MAPPER_PUBLIC_ASSETS, 'sprite_manifest.json') },
    { src: path.join(CLIENT_ASSETS_DIR, 'animated_sprites.json'), dest: path.join(MAPPER_PUBLIC_ASSETS, 'animated_sprites.json') },
  ];

  // Copy atlas PNGs
  for (const cat of ['objects', 'walls']) {
    const atlasName = `${cat}_atlas.png`;
    filesToCopy.push({
      src: path.join(CLIENT_SPRITES_DIR, atlasName),
      dest: path.join(MAPPER_SPRITES_DIR, atlasName),
    });
  }

  // Copy tiles.png
  filesToCopy.push({
    src: path.join(CLIENT_SPRITES_DIR, 'tiles.png'),
    dest: path.join(MAPPER_SPRITES_DIR, 'tiles.png'),
  });

  for (const { src, dest } of filesToCopy) {
    try {
      await fs.copyFile(src, dest);
    } catch (err) {
      console.warn(`  Warning: Could not copy ${src} -> ${dest}: ${(err as Error).message}`);
    }
  }
}

async function runAtlasRebuild(): Promise<{ success: boolean; duration: number; error?: string }> {
  const start = Date.now();
  try {
    console.log('[Atlas Rebuild] Starting...');
    await execAsync(`cd "${projectRoot}" && python3 tools/detect_animated_sprites.py`, { timeout: 120000 });
    await execAsync(`cd "${projectRoot}" && python3 tools/pack_atlases.py`, { timeout: 120000 });
    await copyAssetToMapper();
    const duration = Date.now() - start;
    console.log(`[Atlas Rebuild] Complete in ${duration}ms`);
    return { success: true, duration };
  } catch (err) {
    const duration = Date.now() - start;
    const message = (err as Error).message;
    console.error(`[Atlas Rebuild] Failed: ${message}`);
    return { success: false, duration, error: message };
  }
}

async function runTilesheetRebuild(): Promise<{ success: boolean; duration: number; error?: string }> {
  const start = Date.now();
  try {
    console.log('[Tilesheet Rebuild] Starting...');
    await execAsync(`cd "${projectRoot}" && python3 tools/tiles_sheet.py reconstruct`, { timeout: 60000 });
    // Copy rebuilt tiles.png to mapper
    await fs.copyFile(
      path.join(CLIENT_SPRITES_DIR, 'tiles.png'),
      path.join(MAPPER_SPRITES_DIR, 'tiles.png')
    );
    const duration = Date.now() - start;
    console.log(`[Tilesheet Rebuild] Complete in ${duration}ms`);
    return { success: true, duration };
  } catch (err) {
    const duration = Date.now() - start;
    const message = (err as Error).message;
    console.error(`[Tilesheet Rebuild] Failed: ${message}`);
    return { success: false, duration, error: message };
  }
}

// Get next available ID for a category
app.get('/api/assets/next-id/:category', async (req, res) => {
  try {
    const { category } = req.params;
    if (!['objects', 'walls', 'tiles'].includes(category)) {
      return res.status(400).json({ error: 'Invalid category. Must be objects, walls, or tiles' });
    }
    const nextId = await getNextId(category);
    res.json({ nextId });
  } catch (err) {
    console.error('Error getting next ID:', err);
    res.status(500).json({ error: 'Failed to get next ID' });
  }
});

// Upload a single asset
app.post('/api/assets/upload', upload.single('file') as any, async (req, res) => {
  try {
    const file = req.file;
    if (!file) return res.status(400).json({ error: 'No file uploaded' });

    const category = req.body.category as string;
    if (!['objects', 'walls', 'tiles'].includes(category)) {
      await fs.unlink(file.path);
      return res.status(400).json({ error: 'Invalid category' });
    }

    // Get dimensions
    const dimensions = await readPngDimensions(file.path);
    if (!dimensions.width || !dimensions.height) {
      await fs.unlink(file.path);
      return res.status(400).json({ error: 'Could not read image dimensions' });
    }

    const width = dimensions.width;
    const height = dimensions.height;

    // Handle tiles differently
    if (category === 'tiles') {
      // Validate tile dimensions (must be 64xN*32 where N >= 1)
      if (width % 64 !== 0 || height !== 32) {
        // Allow single tiles (64x32) or strips (multiple of 64 wide)
        if (height !== 32) {
          await fs.unlink(file.path);
          return res.status(400).json({ error: `Tile height must be 32px, got ${height}px` });
        }
        if (width % 64 !== 0) {
          await fs.unlink(file.path);
          return res.status(400).json({ error: `Tile width must be a multiple of 64px, got ${width}px` });
        }
      }

      // Find next tile index
      await fs.mkdir(TILES_EXTRACTED_DIR, { recursive: true });
      const existingTiles = await fs.readdir(TILES_EXTRACTED_DIR);
      let maxIdx = -1;
      for (const f of existingTiles) {
        const m = f.match(/^tile_(\d+)\.png$/);
        if (m) maxIdx = Math.max(maxIdx, parseInt(m[1]));
      }

      const tileCount = width / 64;
      const newTileIds: number[] = [];

      if (tileCount === 1) {
        // Single tile
        const idx = maxIdx + 1;
        const tilePath = path.join(TILES_EXTRACTED_DIR, `tile_${String(idx).padStart(4, '0')}.png`);
        await fs.copyFile(file.path, tilePath);
        newTileIds.push(idx);
      } else {
        // Strip of tiles — split them. Use a simple approach with exec
        for (let i = 0; i < tileCount; i++) {
          const idx = maxIdx + 1 + i;
          newTileIds.push(idx);
        }
        // We need to split the image. Use a temp approach with the raw buffer
        // Since we don't want heavy deps, we'll write each tile individually
        // by reading the file and cropping via a canvas... but we're on server.
        // Let's use Python for this since we already have PIL available.
        const srcPath = file.path;
        for (let i = 0; i < tileCount; i++) {
          const idx = maxIdx + 1 + i;
          const tilePath = path.join(TILES_EXTRACTED_DIR, `tile_${String(idx).padStart(4, '0')}.png`);
          await execAsync(
            `python3 -c "from PIL import Image; img=Image.open('${srcPath}'); img.crop((${i * 64},0,${(i + 1) * 64},32)).save('${tilePath}')"`,
            { timeout: 10000 }
          );
        }
      }

      await fs.unlink(file.path);

      // Also copy individual tile files to mapper public for preview
      const mapperTilesDir = path.join(MAPPER_SPRITES_DIR, 'tiles_preview');
      await fs.mkdir(mapperTilesDir, { recursive: true });
      for (const idx of newTileIds) {
        const src = path.join(TILES_EXTRACTED_DIR, `tile_${String(idx).padStart(4, '0')}.png`);
        const dest = path.join(mapperTilesDir, `tile_${idx}.png`);
        await fs.copyFile(src, dest);
      }

      // Rebuild tilesheet in background
      runTilesheetRebuild().then(result => {
        if (result.success) {
          // Update mapper-config tileCount if needed
          readMapperConfig().then(config => {
            // tileCount is computed from image dimensions, so no config update needed
            // The client re-reads the image
          }).catch(() => {});
        }
      });

      return res.json({
        category: 'tiles',
        tileIds: newTileIds,
        count: newTileIds.length,
        width: 64,
        height: 32,
      });
    }

    // Objects / Walls
    let id = req.body.id ? parseInt(req.body.id) : await getNextId(category);
    const name = req.body.name || String(id);

    // Ensure directories exist
    const clientDir = path.join(CLIENT_SPRITES_DIR, category);
    const mapperDir = path.join(MAPPER_SPRITES_DIR, category);
    await fs.mkdir(clientDir, { recursive: true });
    await fs.mkdir(mapperDir, { recursive: true });

    // Save PNG to both locations
    const filename = `${id}.png`;
    await fs.copyFile(file.path, path.join(clientDir, filename));
    await fs.copyFile(file.path, path.join(mapperDir, filename));
    await fs.unlink(file.path);

    // Update mapper-config.json
    const config = await readMapperConfig();
    const section = category === 'objects' ? config.objects : config.walls;
    if (section) {
      // Check if ID already exists
      const existing = section.items.findIndex(item => item.id === id);
      if (existing >= 0) {
        section.items[existing] = { id, name, width, height };
      } else {
        section.items.push({ id, name, width, height });
        // Sort by ID for cleanliness
        section.items.sort((a, b) => a.id - b.id);
      }
    }
    await writeMapperConfig(config);

    // Handle animation
    let animation: { frames: number; fps: number } | null = null;
    if (req.body.animation) {
      try {
        animation = JSON.parse(req.body.animation);
      } catch { /* ignore */ }
    }

    if (animation) {
      // Update animated_sprites.json
      const animPath = path.join(CLIENT_ASSETS_DIR, 'animated_sprites.json');
      let animData: Record<string, Record<string, { frames: number; fps: number }>> = { objects: {}, walls: {} };
      try {
        const raw = await fs.readFile(animPath, 'utf-8');
        animData = JSON.parse(raw);
      } catch { /* start fresh */ }
      animData[category] = animData[category] || {};
      animData[category][String(id)] = { frames: animation.frames, fps: animation.fps };
      await fs.writeFile(animPath, JSON.stringify(animData, null, 2) + '\n');
      // Also update mapper copy
      const mapperAnimPath = path.join(MAPPER_PUBLIC_ASSETS, 'animated_sprites.json');
      await fs.writeFile(mapperAnimPath, JSON.stringify(animData, null, 2) + '\n');
    }

    // Run atlas rebuild in background
    runAtlasRebuild().catch(err => console.error('Background atlas rebuild error:', err));

    res.json({ id, name, width, height, animation, category });
  } catch (err) {
    console.error('Upload error:', err);
    if (req.file) await fs.unlink(req.file.path).catch(() => {});
    res.status(500).json({ error: `Upload failed: ${(err as Error).message}` });
  }
});

// Batch upload
app.post('/api/assets/upload-batch', upload.array('files', 50) as any, async (req, res) => {
  try {
    const files = req.files as Express.Multer.File[];
    if (!files || files.length === 0) return res.status(400).json({ error: 'No files uploaded' });

    const category = req.body.category as string;
    if (!['objects', 'walls'].includes(category)) {
      for (const f of files) await fs.unlink(f.path).catch(() => {});
      return res.status(400).json({ error: 'Batch upload only supports objects and walls' });
    }

    const config = await readMapperConfig();
    const section = category === 'objects' ? config.objects : config.walls;
    if (!section) {
      for (const f of files) await fs.unlink(f.path).catch(() => {});
      return res.status(400).json({ error: `No ${category} config section found` });
    }

    const clientDir = path.join(CLIENT_SPRITES_DIR, category);
    const mapperDir = path.join(MAPPER_SPRITES_DIR, category);
    await fs.mkdir(clientDir, { recursive: true });
    await fs.mkdir(mapperDir, { recursive: true });

    let nextId = await getNextId(category);
    const results: Array<{ id: number; name: string; width: number; height: number }> = [];

    for (const file of files) {
      const dimensions = await readPngDimensions(file.path);
      if (!dimensions.width || !dimensions.height) {
        await fs.unlink(file.path);
        continue;
      }

      const id = nextId++;
      const name = file.originalname.replace(/\.png$/i, '') || String(id);
      const filename = `${id}.png`;

      await fs.copyFile(file.path, path.join(clientDir, filename));
      await fs.copyFile(file.path, path.join(mapperDir, filename));
      await fs.unlink(file.path);

      section.items.push({ id, name, width: dimensions.width, height: dimensions.height });
      results.push({ id, name, width: dimensions.width, height: dimensions.height });
    }

    section.items.sort((a, b) => a.id - b.id);
    await writeMapperConfig(config);

    // Run atlas rebuild in background
    runAtlasRebuild().catch(err => console.error('Background atlas rebuild error:', err));

    res.json({ results, category });
  } catch (err) {
    console.error('Batch upload error:', err);
    res.status(500).json({ error: `Batch upload failed: ${(err as Error).message}` });
  }
});

// Detect animation in a single file
app.post('/api/assets/detect-animation', upload.single('file') as any, async (req, res) => {
  try {
    const file = req.file;
    if (!file) return res.status(400).json({ error: 'No file uploaded' });

    try {
      const { stdout } = await execAsync(
        `cd "${projectRoot}" && python3 tools/detect_animated_sprites.py --single "${file.path}"`,
        { timeout: 10000 }
      );
      const result = JSON.parse(stdout.trim());
      await fs.unlink(file.path);
      res.json(result);
    } catch {
      await fs.unlink(file.path);
      res.json(null);
    }
  } catch (err) {
    if (req.file) await fs.unlink(req.file.path).catch(() => {});
    res.status(500).json({ error: 'Animation detection failed' });
  }
});

// Delete an asset (soft delete)
app.delete('/api/assets/:category/:id', async (req, res) => {
  try {
    const { category, id } = req.params;
    if (!['objects', 'walls'].includes(category)) {
      return res.status(400).json({ error: 'Invalid category' });
    }

    const numId = parseInt(id);
    if (isNaN(numId)) return res.status(400).json({ error: 'Invalid ID' });

    // Move file to _deleted directory
    const deletedDir = path.join(CLIENT_SPRITES_DIR, '_deleted', category);
    await fs.mkdir(deletedDir, { recursive: true });

    const filename = `${numId}.png`;
    const clientSrc = path.join(CLIENT_SPRITES_DIR, category, filename);
    const mapperSrc = path.join(MAPPER_SPRITES_DIR, category, filename);

    try {
      await fs.rename(clientSrc, path.join(deletedDir, filename));
    } catch { /* may not exist */ }
    try {
      await fs.unlink(mapperSrc);
    } catch { /* may not exist */ }

    // Remove from config
    const config = await readMapperConfig();
    const section = category === 'objects' ? config.objects : config.walls;
    if (section) {
      section.items = section.items.filter(item => item.id !== numId);
    }
    await writeMapperConfig(config);

    // Rebuild atlas in background
    runAtlasRebuild().catch(err => console.error('Background atlas rebuild error:', err));

    res.json({ success: true, id: numId, category });
  } catch (err) {
    console.error('Delete error:', err);
    res.status(500).json({ error: `Delete failed: ${(err as Error).message}` });
  }
});

// Manual atlas rebuild
app.post('/api/assets/rebuild-atlas', async (_req, res) => {
  try {
    const result = await runAtlasRebuild();
    res.json(result);
  } catch (err) {
    res.status(500).json({ error: `Rebuild failed: ${(err as Error).message}` });
  }
});

// Serve static frontend files (after API routes)
app.use('/mapper', express.static(distPath));
app.use(express.static(distPath));
// Serve individual sprite files from client assets (not in vite dist)
app.use('/assets/sprites', express.static(CLIENT_SPRITES_DIR));

// SPA fallback - serve index.html for non-API routes
app.get('*', (req, res) => {
  // Don't serve HTML for API routes
  if (req.path.startsWith('/api')) {
    return res.status(404).json({ error: 'Not found' });
  }
  res.sendFile(path.join(distPath, 'index.html'));
});

// Start server
async function main() {
  await ensureDataDirs();
  notesCache = await loadNotesFromDisk();
  console.log(`  notes: ${notesCache.length} loaded`);
  console.log('Paths:');
  console.log('  mapperRoot:', mapperRoot);
  console.log('  distPath:', distPath);
  console.log('  dataDir:', DATA_DIR);
  app.listen(PORT, () => {
    console.log(`Mapper server running on http://localhost:${PORT}`);
  });
}

main().catch(console.error);
