import { installMapperAuth } from './auth.js';
import { createMapRouter } from './routes/maps.js';
import { createNotesRouter } from './routes/notes.js';
import { createContentRouter } from './routes/content.js';
import { createAssetRouter } from './routes/assets.js';
import { HttpError } from './http.js';
import express from 'express';
import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const app = express();
const PORT = process.env.PORT || 3000;
const HOST = process.env.MAPPER_HOST || '127.0.0.1';
const IS_PRODUCTION = process.env.NODE_ENV === 'production';

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
  if (!(VALID_WORLDS as readonly string[]).includes(world)) {
    throw new HttpError(400, `Invalid world: ${world}`);
  }
  const username = auth.getUser(req);
  const user = username ? auth.users[username] : undefined;
  if (!user?.worlds.includes(world)) {
    throw new HttpError(403, `Access denied for world: ${world}`);
  }
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
const GAME_DATA_DIR = path.join(projectRoot, 'rust-server', 'data');

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

// Middleware
app.use(express.json({ limit: '50mb' }));
app.use(express.urlencoded({ extended: false }));
app.disable('x-powered-by');
app.use((_req, res, next) => {
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('X-Frame-Options', 'DENY');
  res.setHeader('Referrer-Policy', 'no-referrer');
  res.setHeader('Permissions-Policy', 'camera=(), microphone=(), geolocation=()');
  next();
});

const auth = await installMapperAuth(app, mapperRoot, IS_PRODUCTION);

// Frontend dist path (served after API routes)
const distPath = path.join(mapperRoot, 'dist');

// --- User Info ---
app.get('/api/me', (req, res) => {
  const username = auth.getUser(req) || 'unknown';
  const user = auth.users[username];
  res.json({ username, worlds: user?.worlds || ['world_0'] });
});

// Serve mapper-config.json (live from disk, not from dist cache)
app.get('/mapper-config.json', (_req, res) => {
  res.set('Cache-Control', 'no-store, no-cache, must-revalidate');
  res.sendFile(path.join(mapperRoot, 'mapper-config.json'));
});
app.get('/mapper/mapper-config.json', (_req, res) => {
  res.set('Cache-Control', 'no-store, no-cache, must-revalidate');
  res.sendFile(path.join(mapperRoot, 'mapper-config.json'));
});


const { router: notesRouter, count: notesCount } = await createNotesRouter(NOTES_FILE);
app.use(createMapRouter({
  getWorldDirs,
  getWorldFromRequest,
  gameServerDir: GAME_SERVER_DIR,
}));
app.use(notesRouter);
app.use(createContentRouter(GAME_DATA_DIR));
app.use(createAssetRouter({
  dataDir: DATA_DIR,
  projectRoot,
  clientAssetsDir: CLIENT_ASSETS_DIR,
  clientSpritesDir: CLIENT_SPRITES_DIR,
  mapperPublicAssets: MAPPER_PUBLIC_ASSETS,
  mapperSpritesDir: MAPPER_SPRITES_DIR,
  mapperConfigPath: MAPPER_CONFIG_PATH,
  tilesExtractedDir: TILES_EXTRACTED_DIR,
}));

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
  console.log(`  notes: ${notesCount} loaded`);
  console.log('Paths:');
  console.log('  mapperRoot:', mapperRoot);
  console.log('  distPath:', distPath);
  console.log('  dataDir:', DATA_DIR);
  app.listen(Number(PORT), HOST, () => {
    console.log(`Mapper server running on http://${HOST}:${PORT}`);
  });
}

main().catch(console.error);
