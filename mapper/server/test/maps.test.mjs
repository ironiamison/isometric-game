import assert from 'node:assert/strict';
import { once } from 'node:events';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import express from 'express';

import { createMapRouter } from '../dist/routes/maps.js';

function chunk(cx, cy, ground = 1) {
  const tileCount = 4;
  return {
    coord: { cx, cy },
    width: 2,
    height: 2,
    layers: {
      ground: Array(tileCount).fill(ground),
      objects: Array(tileCount).fill(0),
      overhead: Array(tileCount).fill(0),
    },
    collision: [0],
  };
}

async function startMapServer(root) {
  const worldRoot = path.join(root, 'world_0');
  const chunksDir = path.join(worldRoot, 'chunks');
  const interiorsDir = path.join(worldRoot, 'interiors');
  await fs.mkdir(chunksDir, { recursive: true });
  await fs.mkdir(interiorsDir, { recursive: true });

  const app = express();
  app.use(express.json());
  app.use(createMapRouter({
    getWorldDirs: () => ({
      chunksDir,
      interiorsDir,
      gameChunksDir: path.join(root, 'game', 'world_0'),
      gameInteriorsDir: path.join(root, 'game', 'interiors'),
    }),
    getWorldFromRequest: () => 'world_0',
    gameServerDir: path.join(root, 'game'),
  }));
  const server = app.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert(address && typeof address === 'object');
  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    chunksDir,
    close: () => new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    }),
  };
}

test('map imports validate complete chunk shapes and replace the prior set', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-maps-'));
  const server = await startMapServer(root);
  try {
    await fs.writeFile(
      path.join(server.chunksDir, '99_99.json'),
      JSON.stringify(chunk(99, 99)),
    );

    const malformed = chunk(1, 1);
    malformed.layers.ground.pop();
    const rejected = await fetch(`${server.baseUrl}/api/map/import`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ chunks: { '1,1': malformed } }),
    });
    assert.equal(rejected.status, 400);
    assert.deepEqual(await fs.readdir(server.chunksDir), ['99_99.json']);

    const imported = await fetch(`${server.baseUrl}/api/map/import`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        chunks: {
          '1,1': chunk(1, 1),
          '-2,3': chunk(-2, 3, 2),
        },
      }),
    });
    assert.equal(imported.status, 200);
    assert.deepEqual(
      (await fs.readdir(server.chunksDir)).sort(),
      ['-2_3.json', '1_1.json'],
    );

    const loaded = JSON.parse(await fs.readFile(
      path.join(server.chunksDir, '-2_3.json'),
      'utf-8',
    ));
    assert.deepEqual(loaded, chunk(-2, 3, 2));
    const leftovers = (await fs.readdir(path.dirname(server.chunksDir)))
      .filter((name) => name.includes('.staging') || name.includes('.backup'));
    assert.deepEqual(leftovers, []);
  } finally {
    await server.close();
    await fs.rm(root, { recursive: true, force: true });
  }
});

test('single chunk writes reject coordinate and collision mismatches', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-map-single-'));
  const server = await startMapServer(root);
  try {
    const wrongCoordinate = await fetch(`${server.baseUrl}/api/chunks/2/2`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(chunk(1, 1)),
    });
    assert.equal(wrongCoordinate.status, 400);

    const invalidCollision = chunk(2, 2);
    invalidCollision.collision = [];
    const wrongCollision = await fetch(`${server.baseUrl}/api/chunks/2/2`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(invalidCollision),
    });
    assert.equal(wrongCollision.status, 400);
  } finally {
    await server.close();
    await fs.rm(root, { recursive: true, force: true });
  }
});
