import assert from 'node:assert/strict';
import { once } from 'node:events';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import express from 'express';

import { createAssetRouter } from '../dist/routes/assets.js';

function pngHeader(width = 64, height = 32) {
  const header = Buffer.alloc(24);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(header);
  header.writeUInt32BE(13, 8);
  header.write('IHDR', 12, 'ascii');
  header.writeUInt32BE(width, 16);
  header.writeUInt32BE(height, 20);
  return header;
}

async function startAssetServer(root) {
  const dataDir = path.join(root, 'data');
  const clientAssetsDir = path.join(root, 'client-assets');
  const clientSpritesDir = path.join(clientAssetsDir, 'sprites');
  const mapperPublicAssets = path.join(root, 'mapper-public-assets');
  const mapperSpritesDir = path.join(mapperPublicAssets, 'sprites');
  const mapperConfigPath = path.join(root, 'mapper-config.json');
  const uploadsDir = path.join(dataDir, 'uploads');
  await Promise.all([
    fs.mkdir(uploadsDir, { recursive: true }),
    fs.mkdir(clientSpritesDir, { recursive: true }),
    fs.mkdir(mapperSpritesDir, { recursive: true }),
    fs.mkdir(mapperPublicAssets, { recursive: true }),
  ]);
  const config = {
    tilesets: [],
    objects: { basePath: 'sprites/objects', firstGid: 1, items: [] },
    walls: { basePath: 'sprites/walls', firstGid: 1, items: [] },
    chunkSize: 16,
    mapsPath: 'maps',
    entitiesPath: 'entities',
  };
  await fs.writeFile(mapperConfigPath, JSON.stringify(config));

  const app = express();
  app.use(createAssetRouter({
    dataDir,
    projectRoot: root,
    clientAssetsDir,
    clientSpritesDir,
    mapperPublicAssets,
    mapperSpritesDir,
    mapperConfigPath,
    tilesExtractedDir: path.join(root, 'tiles-extracted'),
  }));
  const server = app.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert(address && typeof address === 'object');
  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    clientSpritesDir,
    mapperConfigPath,
    uploadsDir,
    close: () => new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    }),
  };
}

async function postAsset(baseUrl, route, fields, files) {
  const form = new FormData();
  for (const [name, value] of Object.entries(fields)) {
    form.set(name, value);
  }
  for (const { field, contents, name } of files) {
    form.append(field, new Blob([contents], { type: 'image/png' }), name);
  }
  return fetch(`${baseUrl}${route}`, { method: 'POST', body: form });
}

test('asset uploads reject malformed PNGs without mutating content', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-assets-invalid-'));
  const server = await startAssetServer(root);
  try {
    const response = await postAsset(
      server.baseUrl,
      '/api/assets/upload',
      { category: 'objects' },
      [{ field: 'file', contents: Buffer.alloc(24), name: 'invalid.png' }],
    );
    assert.equal(response.status, 400);
    assert.deepEqual(await fs.readdir(server.uploadsDir), []);
    await assert.rejects(fs.access(path.join(server.clientSpritesDir, 'objects', '1.png')));
    const config = JSON.parse(await fs.readFile(server.mapperConfigPath, 'utf-8'));
    assert.deepEqual(config.objects.items, []);
  } finally {
    await server.close();
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('animation metadata is validated before files or config are changed', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-assets-animation-'));
  const server = await startAssetServer(root);
  try {
    const response = await postAsset(
      server.baseUrl,
      '/api/assets/upload',
      { category: 'objects', id: '7', animation: '{"frames":3,"fps":12}' },
      [{ field: 'file', contents: pngHeader(), name: 'object.png' }],
    );
    assert.equal(response.status, 400);
    await assert.rejects(fs.access(path.join(server.clientSpritesDir, 'objects', '7.png')));
    const config = JSON.parse(await fs.readFile(server.mapperConfigPath, 'utf-8'));
    assert.deepEqual(config.objects.items, []);
  } finally {
    await server.close();
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('batch upload validates every file before applying any file', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-assets-batch-'));
  const server = await startAssetServer(root);
  try {
    const response = await postAsset(
      server.baseUrl,
      '/api/assets/upload-batch',
      { category: 'walls' },
      [
        { field: 'files', contents: pngHeader(), name: 'valid.png' },
        { field: 'files', contents: Buffer.alloc(24), name: 'invalid.png' },
      ],
    );
    assert.equal(response.status, 400);
    assert.deepEqual(await fs.readdir(server.uploadsDir), []);
    await assert.rejects(fs.access(path.join(server.clientSpritesDir, 'walls', '1.png')));
    const config = JSON.parse(await fs.readFile(server.mapperConfigPath, 'utf-8'));
    assert.deepEqual(config.walls.items, []);
  } finally {
    await server.close();
    await fs.rm(root, { recursive: true, force: true });
  }
});
