import assert from 'node:assert/strict';
import { once } from 'node:events';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import express from 'express';

import { createNotesRouter } from '../dist/routes/notes.js';

async function startNotesServer(notesFile) {
  const app = express();
  app.use(express.json());
  const { router } = await createNotesRouter(notesFile);
  app.use(router);
  const server = app.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const address = server.address();
  assert(address && typeof address === 'object');
  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    close: () => new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    }),
  };
}

function note(overrides = {}) {
  return {
    id: 'note_1',
    x: 10,
    y: -5,
    chunkCoord: { cx: 0, cy: -1 },
    text: 'Check this area',
    category: 'todo',
    priority: 'medium',
    status: 'open',
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    ...overrides,
  };
}

test('note writes are validated, serialized, and persisted atomically', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-notes-'));
  const notesFile = path.join(directory, 'notes.json');
  const server = await startNotesServer(notesFile);

  try {
    const created = await fetch(`${server.baseUrl}/api/notes`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(note()),
    });
    assert.equal(created.status, 201);

    const duplicate = await fetch(`${server.baseUrl}/api/notes`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(note()),
    });
    assert.equal(duplicate.status, 409);

    const invalid = await fetch(`${server.baseUrl}/api/notes`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(note({ id: '../escape' })),
    });
    assert.equal(invalid.status, 400);

    const [priorityUpdate, statusUpdate] = await Promise.all([
      fetch(`${server.baseUrl}/api/notes/note_1`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ priority: 'high' }),
      }),
      fetch(`${server.baseUrl}/api/notes/note_1`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ status: 'resolved' }),
      }),
    ]);
    assert.equal(priorityUpdate.status, 200);
    assert.equal(statusUpdate.status, 200);

    const response = await fetch(`${server.baseUrl}/api/notes/note_1`);
    assert.equal(response.status, 200);
    const updated = await response.json();
    assert.equal(updated.priority, 'high');
    assert.equal(updated.status, 'resolved');

    const persisted = JSON.parse(await fs.readFile(notesFile, 'utf-8'));
    assert.deepEqual(persisted, [updated]);
    const leftovers = (await fs.readdir(directory)).filter((name) => name.endsWith('.tmp'));
    assert.deepEqual(leftovers, []);
  } finally {
    await server.close();
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test('invalid persisted notes fail startup instead of being silently accepted', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'aeven-notes-invalid-'));
  const notesFile = path.join(directory, 'notes.json');
  await fs.writeFile(notesFile, JSON.stringify([note({ text: '' })]));
  try {
    await assert.rejects(createNotesRouter(notesFile), /invalid note at index 0/);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
});
