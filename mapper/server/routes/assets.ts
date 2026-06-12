import { exec } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { Router } from 'express';
import multer from 'multer';

const execAsync = promisify(exec);

async function readPngDimensions(filePath: string): Promise<{ width: number; height: number }> {
  const fd = await fs.open(filePath, 'r');
  try {
    const buffer = Buffer.alloc(24);
    await fd.read(buffer, 0, 24, 0);
    if (buffer[0] !== 0x89 || buffer[1] !== 0x50 || buffer[2] !== 0x4e || buffer[3] !== 0x47) {
      throw new Error('Not a valid PNG file');
    }
    return {
      width: buffer.readUInt32BE(16),
      height: buffer.readUInt32BE(20),
    };
  } finally {
    await fd.close();
  }
}

export interface AssetRouterDependencies {
  dataDir: string;
  projectRoot: string;
  clientAssetsDir: string;
  clientSpritesDir: string;
  mapperPublicAssets: string;
  mapperSpritesDir: string;
  mapperConfigPath: string;
  tilesExtractedDir: string;
}

export function createAssetRouter(dependencies: AssetRouterDependencies): Router {
  const router = Router();
  const upload = multer({
    dest: path.join(dependencies.dataDir, 'uploads'),
    limits: { fileSize: 20 * 1024 * 1024, files: 50 },
    fileFilter: (_req, file, callback) => {
      callback(null, file.mimetype === 'image/png');
    },
  });
  const {
    projectRoot,
    clientAssetsDir: CLIENT_ASSETS_DIR,
    clientSpritesDir: CLIENT_SPRITES_DIR,
    mapperPublicAssets: MAPPER_PUBLIC_ASSETS,
    mapperSpritesDir: MAPPER_SPRITES_DIR,
    mapperConfigPath: MAPPER_CONFIG_PATH,
    tilesExtractedDir: TILES_EXTRACTED_DIR,
  } = dependencies;

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
router.get('/api/assets/next-id/:category', async (req, res) => {
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
router.post('/api/assets/upload', upload.single('file'), async (req, res) => {
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

      // Ensure tiles_extracted/ is populated from the current tiles.png
      // so that reconstruct doesn't wipe existing tiles
      await fs.mkdir(TILES_EXTRACTED_DIR, { recursive: true });
      const existingTiles = await fs.readdir(TILES_EXTRACTED_DIR);
      const hasTileFiles = existingTiles.some(f => /^tile_\d+\.png$/.test(f));
      if (!hasTileFiles) {
        const tilesPath = path.join(CLIENT_SPRITES_DIR, 'tiles.png');
        try {
          await fs.access(tilesPath);
          console.log('[Tile Import] tiles_extracted/ is empty, extracting from tiles.png first...');
          await execAsync(
            `cd "${projectRoot}" && python3 tools/tiles_sheet.py extract --input "${tilesPath}" --output "${TILES_EXTRACTED_DIR}"`,
            { timeout: 60000 }
          );
        } catch {
          // tiles.png doesn't exist yet — first-time import, nothing to extract
        }
      }
      // Re-read after potential extraction to get accurate max index
      const tileFiles = await fs.readdir(TILES_EXTRACTED_DIR);
      let maxIdx = -1;
      for (const f of tileFiles) {
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
          // tileCount is computed from image dimensions, so the client only
          // needs to reload the rebuilt image.
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
    const id = req.body.id ? parseInt(req.body.id) : await getNextId(category);
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
router.post('/api/assets/upload-batch', upload.array('files', 50), async (req, res) => {
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
router.post('/api/assets/detect-animation', upload.single('file'), async (req, res) => {
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
  } catch {
    if (req.file) await fs.unlink(req.file.path).catch(() => {});
    res.status(500).json({ error: 'Animation detection failed' });
  }
});

// Delete an asset (soft delete)
router.delete('/api/assets/:category/:id', async (req, res) => {
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
router.post('/api/assets/rebuild-atlas', async (_req, res) => {
  try {
    const result = await runAtlasRebuild();
    res.json(result);
  } catch (err) {
    res.status(500).json({ error: `Rebuild failed: ${(err as Error).message}` });
  }
});


  return router;
}
