import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { once } from 'node:events';
import test from 'node:test';

import express from 'express';

import { installMapperAuth } from '../dist/auth.js';

function passwordHash(password) {
  const salt = 'test-salt';
  const n = 16_384;
  const r = 8;
  const p = 1;
  const expected = crypto.scryptSync(password, salt, 64, {
    N: n,
    r,
    p,
    maxmem: 128 * 1024 * 1024,
  }).toString('base64url');
  return `scrypt$${n}$${r}$${p}$${salt}$${expected}`;
}

async function startServer(app) {
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

test('mapper auth enforces sessions, CSRF, world validation, and login throttling', async () => {
  const previous = {
    users: process.env.MAPPER_USERS,
    secret: process.env.MAPPER_AUTH_SECRET,
  };
  process.env.MAPPER_USERS = JSON.stringify({
    editor: {
      worlds: ['world_0'],
      passwordHash: passwordHash('correct horse battery staple'),
    },
  });
  process.env.MAPPER_AUTH_SECRET = 'test-secret-that-is-at-least-32-characters';

  const app = express();
  app.use(express.urlencoded({ extended: false }));
  app.use(express.json());
  await installMapperAuth(app, '.', true, ['world_0', 'world_1']);
  app.get('/api/probe', (_req, res) => res.json({ ok: true }));
  app.post('/api/probe', (_req, res) => res.json({ ok: true }));

  const server = await startServer(app);
  try {
    const unauthorized = await fetch(`${server.baseUrl}/api/probe`);
    assert.equal(unauthorized.status, 401);

    const login = await fetch(`${server.baseUrl}/mapper/login`, {
      method: 'POST',
      redirect: 'manual',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        username: 'editor',
        password: 'correct horse battery staple',
      }),
    });
    assert.equal(login.status, 302);
    const setCookies = login.headers.getSetCookie();
    assert.equal(setCookies.length, 2);
    const cookie = setCookies.map((value) => value.split(';', 1)[0]).join('; ');
    const csrf = /(?:^|; )mapper_csrf=([^;]+)/.exec(cookie)?.[1];
    assert(csrf);

    const authorized = await fetch(`${server.baseUrl}/api/probe`, {
      headers: { cookie },
    });
    assert.equal(authorized.status, 200);

    const missingCsrf = await fetch(`${server.baseUrl}/api/probe`, {
      method: 'POST',
      headers: { cookie },
    });
    assert.equal(missingCsrf.status, 403);

    const validCsrf = await fetch(`${server.baseUrl}/api/probe`, {
      method: 'POST',
      headers: { cookie, 'x-csrf-token': csrf },
    });
    assert.equal(validCsrf.status, 200);

    for (let attempt = 0; attempt < 5; attempt += 1) {
      const failed = await fetch(`${server.baseUrl}/mapper/login`, {
        method: 'POST',
        redirect: 'manual',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ username: 'editor', password: 'wrong' }),
      });
      assert.equal(failed.status, 401);
    }
    const throttled = await fetch(`${server.baseUrl}/mapper/login`, {
      method: 'POST',
      redirect: 'manual',
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ username: 'editor', password: 'wrong' }),
    });
    assert.equal(throttled.status, 429);
    assert(Number(throttled.headers.get('retry-after')) > 0);
  } finally {
    await server.close();
    if (previous.users === undefined) delete process.env.MAPPER_USERS;
    else process.env.MAPPER_USERS = previous.users;
    if (previous.secret === undefined) delete process.env.MAPPER_AUTH_SECRET;
    else process.env.MAPPER_AUTH_SECRET = previous.secret;
  }
});

test('mapper auth rejects invalid world grants during startup', async () => {
  const previousUsers = process.env.MAPPER_USERS;
  const previousSecret = process.env.MAPPER_AUTH_SECRET;
  process.env.MAPPER_USERS = JSON.stringify({
    editor: {
      worlds: ['unknown_world'],
      passwordHash: passwordHash('password'),
    },
  });
  process.env.MAPPER_AUTH_SECRET = 'test-secret-that-is-at-least-32-characters';

  try {
    await assert.rejects(
      installMapperAuth(express(), '.', true, ['world_0']),
      /invalid world access/,
    );
  } finally {
    if (previousUsers === undefined) delete process.env.MAPPER_USERS;
    else process.env.MAPPER_USERS = previousUsers;
    if (previousSecret === undefined) delete process.env.MAPPER_AUTH_SECRET;
    else process.env.MAPPER_AUTH_SECRET = previousSecret;
  }
});
