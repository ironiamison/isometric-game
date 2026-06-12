import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { Express, Request } from 'express';

export interface MapperUser {
  passwordHash?: string;
  password?: string;
  worlds: string[];
}

export interface MapperAuth {
  users: Record<string, MapperUser>;
  getUser(req: Request): string | undefined;
}

export async function installMapperAuth(
  app: Express,
  mapperRoot: string,
  isProduction: boolean,
  validWorlds: readonly string[],
): Promise<MapperAuth> {
  // --- Auth ---
  type AuthenticatedRequest = Request & { mapperUser?: string };
  
  function getMapperUser(req: Request): string | undefined {
    return (req as AuthenticatedRequest).mapperUser;
  }
  
  function setMapperUser(req: Request, username: string): void {
    (req as AuthenticatedRequest).mapperUser = username;
  }
  
  const usersSource =
    process.env.MAPPER_USERS
    ?? await fs.readFile(path.join(mapperRoot, 'users.json'), 'utf-8');
  const parsedUsers: unknown = JSON.parse(usersSource);
  if (typeof parsedUsers !== 'object' || parsedUsers === null || Array.isArray(parsedUsers)) {
    throw new Error('Mapper users configuration must be an object');
  }

  const users: Record<string, MapperUser> = {};
  for (const [username, value] of Object.entries(parsedUsers)) {
    if (!/^[a-zA-Z0-9_.-]{1,64}$/.test(username)) {
      throw new Error(`Invalid mapper username: ${username}`);
    }
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      throw new Error(`Invalid mapper user configuration: ${username}`);
    }
    const candidate = value as Partial<MapperUser>;
    if (
      !Array.isArray(candidate.worlds)
      || candidate.worlds.length === 0
      || candidate.worlds.some(
        (world) => typeof world !== 'string' || !validWorlds.includes(world),
      )
    ) {
      throw new Error(`Mapper user ${username} has invalid world access`);
    }
    if (new Set(candidate.worlds).size !== candidate.worlds.length) {
      throw new Error(`Mapper user ${username} has duplicate world access`);
    }
    if (isProduction && typeof candidate.passwordHash !== 'string') {
      throw new Error(`Mapper user ${username} requires passwordHash in production`);
    }
    users[username] = {
      worlds: [...candidate.worlds],
      ...(typeof candidate.passwordHash === 'string'
        ? { passwordHash: candidate.passwordHash }
        : {}),
      ...(typeof candidate.password === 'string' ? { password: candidate.password } : {}),
    };
  }
  if (Object.keys(users).length === 0) {
    throw new Error('At least one mapper user must be configured');
  }
  const configuredAuthSecret = process.env.MAPPER_AUTH_SECRET;
  if (isProduction && (!configuredAuthSecret || configuredAuthSecret.length < 32)) {
    throw new Error('MAPPER_AUTH_SECRET must be set to at least 32 characters in production');
  }
  const AUTH_SECRET = configuredAuthSecret || crypto.randomBytes(32).toString('hex');
  const SESSION_MAX_AGE_SECONDS = 8 * 60 * 60;
  const COOKIE_SECURITY = isProduction ? '; Secure' : '';
  const COOKIE_PRIORITY = '; Priority=High';
  const LOGIN_WINDOW_MS = 15 * 60 * 1000;
  const MAX_LOGIN_FAILURES = 5;
  const loginFailures = new Map<string, { count: number; resetAt: number }>();
  
  function sign(value: string): string {
    return crypto.createHmac('sha256', AUTH_SECRET).update(value).digest('base64url');
  }
  
  function makeToken(username: string): string {
    const payload = Buffer.from(JSON.stringify({
      username,
      expiresAt: Date.now() + SESSION_MAX_AGE_SECONDS * 1000,
      nonce: crypto.randomBytes(16).toString('base64url'),
    })).toString('base64url');
    return `${payload}.${sign(payload)}`;
  }
  
  function getUserFromToken(token: string): string | null {
    const [payload, signature, extra] = token.split('.');
    if (!payload || !signature || extra) return null;
    const expected = sign(payload);
    if (!safeEqual(signature, expected)) return null;
    try {
      const parsed = JSON.parse(Buffer.from(payload, 'base64url').toString('utf-8')) as {
        username?: string;
        expiresAt?: number;
      };
      if (!parsed.username || !users[parsed.username]) return null;
      if (!parsed.expiresAt || parsed.expiresAt <= Date.now()) return null;
      return parsed.username;
    } catch {
      return null;
    }
  }
  
  function safeEqual(left: string, right: string): boolean {
    const leftBytes = Buffer.from(left);
    const rightBytes = Buffer.from(right);
    return leftBytes.length === rightBytes.length && crypto.timingSafeEqual(leftBytes, rightBytes);
  }
  
  function verifyPassword(password: string, user: MapperUser): boolean {
    if (user.passwordHash) {
      const [algorithm, nRaw, rRaw, pRaw, salt, expected] = user.passwordHash.split('$');
      if (algorithm !== 'scrypt' || !nRaw || !rRaw || !pRaw || !salt || !expected) return false;
      const n = Number(nRaw);
      const r = Number(rRaw);
      const p = Number(pRaw);
      if (!Number.isInteger(n) || !Number.isInteger(r) || !Number.isInteger(p)) return false;
      const actual = crypto.scryptSync(password, salt, 64, {
        N: n,
        r,
        p,
        maxmem: 128 * 1024 * 1024,
      }).toString('base64url');
      return safeEqual(actual, expected);
    }
    if (!isProduction && process.env.MAPPER_ALLOW_PLAINTEXT_PASSWORDS === '1' && user.password) {
      return safeEqual(password, user.password);
    }
    return false;
  }

  const dummyPasswordHash = (() => {
    const salt = 'mapper-login-timing-padding';
    const n = 16_384;
    const r = 8;
    const p = 1;
    const expected = crypto.scryptSync('not-a-real-password', salt, 64, {
      N: n,
      r,
      p,
      maxmem: 128 * 1024 * 1024,
    }).toString('base64url');
    return `scrypt$${n}$${r}$${p}$${salt}$${expected}`;
  })();

  function loginKey(req: Request): string {
    return req.ip || req.socket.remoteAddress || 'unknown';
  }

  function currentLoginFailure(key: string): { count: number; resetAt: number } | undefined {
    const failure = loginFailures.get(key);
    if (failure && failure.resetAt <= Date.now()) {
      loginFailures.delete(key);
      return undefined;
    }
    return failure;
  }

  function recordLoginFailure(key: string): void {
    const current = currentLoginFailure(key);
    loginFailures.set(key, {
      count: (current?.count ?? 0) + 1,
      resetAt: current?.resetAt ?? Date.now() + LOGIN_WINDOW_MS,
    });
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
  
  app.post(['/login', '/mapper/login'], (req, res) => {
    const { username, password } = req.body || {};
    const key = loginKey(req);
    const failure = currentLoginFailure(key);
    if (failure && failure.count >= MAX_LOGIN_FAILURES) {
      res.setHeader('Retry-After', Math.ceil((failure.resetAt - Date.now()) / 1000));
      return res
        .status(429)
        .send(LOGIN_HTML.replace(
          'ERRPLACEHOLDER',
          '<p class="error">Too many attempts. Try again later.</p>',
        ));
    }

    const safeUsername =
      typeof username === 'string' && username.length <= 64 ? username : '';
    const safePassword =
      typeof password === 'string' && password.length <= 1024 ? password : '';
    const user = users[safeUsername];
    const passwordMatches = verifyPassword(
      safePassword,
      user ?? { worlds: [], passwordHash: dummyPasswordHash },
    );

    if (user && safePassword && passwordMatches) {
      loginFailures.delete(key);
      const csrfToken = crypto.randomBytes(32).toString('base64url');
      res.setHeader('Set-Cookie', [
        `mapper_token=${makeToken(safeUsername)}; Path=/; HttpOnly; SameSite=Strict; Max-Age=${SESSION_MAX_AGE_SECONDS}${COOKIE_SECURITY}${COOKIE_PRIORITY}`,
        `mapper_csrf=${csrfToken}; Path=/; SameSite=Strict; Max-Age=${SESSION_MAX_AGE_SECONDS}${COOKIE_SECURITY}${COOKIE_PRIORITY}`,
      ]);
      return res.redirect('/mapper/');
    }
    recordLoginFailure(key);
    return res
      .status(401)
      .send(LOGIN_HTML.replace(
        'ERRPLACEHOLDER',
        '<p class="error">Invalid credentials</p>',
      ));
  });
  
  app.get(['/login', '/mapper/login'], (_req, res) => {
    res.send(LOGIN_HTML.replace('ERRPLACEHOLDER', ''));
  });
  
  app.post(['/logout', '/mapper/logout'], (_req, res) => {
    res.setHeader('Set-Cookie', [
      `mapper_token=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0${COOKIE_SECURITY}${COOKIE_PRIORITY}`,
      `mapper_csrf=; Path=/; SameSite=Strict; Max-Age=0${COOKIE_SECURITY}${COOKIE_PRIORITY}`,
    ]);
    res.redirect('/mapper/login');
  });
  
  // Authentication is required for every mapper page and API.
  app.use((req, res, next) => {
    const cookies = parseCookies(req.headers.cookie);
    const user = getUserFromToken(cookies.mapper_token || '');
    if (user) {
      setMapperUser(req, user);
      return next();
    }
    if (req.path.startsWith('/api/') || req.path.startsWith('/mapper/api/')) {
      return res.status(401).json({ error: 'Authentication required' });
    }
    return res.redirect('/mapper/login');
  });
  
  // State-changing API calls require a double-submit CSRF token.
  app.use((req, res, next) => {
    if (['GET', 'HEAD', 'OPTIONS'].includes(req.method)) return next();
    if (!req.path.startsWith('/api/') && !req.path.startsWith('/mapper/api/')) return next();
    const cookies = parseCookies(req.headers.cookie);
    const cookieToken = cookies.mapper_csrf || '';
    const headerToken = req.get('x-csrf-token') || '';
    if (!cookieToken || !headerToken || !safeEqual(cookieToken, headerToken)) {
      return res.status(403).json({ error: 'Invalid CSRF token' });
    }
    return next();
  });
  
  
  return { users, getUser: getMapperUser };
}
