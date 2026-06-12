import crypto from 'node:crypto';

const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const password = Buffer.concat(chunks).toString('utf8').replace(/\r?\n$/, '');
if (password.length < 12) {
  console.error('Password must be at least 12 characters.');
  process.exit(1);
}

const N = 16384;
const r = 8;
const p = 1;
const salt = crypto.randomBytes(16).toString('base64url');
const hash = crypto.scryptSync(password, salt, 64, {
  N,
  r,
  p,
  maxmem: 128 * 1024 * 1024,
}).toString('base64url');
process.stdout.write(`scrypt$${N}$${r}$${p}$${salt}$${hash}\n`);
