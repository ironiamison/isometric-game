import { Router } from 'express';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import { parse as parseTOML, stringify as stringifyTOML } from 'smol-toml';

export function createContentRouter(gameDataDir: string): Router {
  const router = Router();
  const GAME_DATA_DIR = gameDataDir;

// --- Content Studio API ---

type ContentKind = 'item' | 'enemy' | 'npc' | 'attack';

interface ContentFileDescriptor {
  path: string;
  kind: ContentKind;
}

const CONTENT_DIRECTORIES: Array<{ directory: string; kind: ContentKind }> = [
  { directory: 'items', kind: 'item' },
  { directory: 'entities/monsters', kind: 'enemy' },
  { directory: 'entities/npcs', kind: 'npc' },
  { directory: 'spells', kind: 'attack' },
];

function isValidContentId(id: string): boolean {
  return /^[a-z][a-z0-9_]*$/.test(id);
}

function resolveContentPath(relativePath: string): string {
  const normalized = relativePath.replaceAll('\\', '/');
  const descriptor = CONTENT_DIRECTORIES.find(
    ({ directory }) => normalized.startsWith(`${directory}/`) && normalized.endsWith('.toml')
  );
  if (!descriptor || normalized.includes('..')) {
    throw new Error('Invalid content file path');
  }

  const absolutePath = path.resolve(GAME_DATA_DIR, normalized);
  const dataRoot = path.resolve(GAME_DATA_DIR) + path.sep;
  if (!absolutePath.startsWith(dataRoot)) {
    throw new Error('Content file path escapes the data directory');
  }
  return absolutePath;
}

async function listContentFiles(): Promise<ContentFileDescriptor[]> {
  const files: ContentFileDescriptor[] = [];
  for (const { directory, kind } of CONTENT_DIRECTORIES) {
    const absoluteDirectory = path.join(GAME_DATA_DIR, directory);
    let entries: string[] = [];
    try {
      entries = await fs.readdir(absoluteDirectory);
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') throw err;
    }

    for (const filename of entries.sort()) {
      if (filename.endsWith('.toml')) {
        files.push({ path: `${directory}/${filename}`, kind });
      }
    }
  }
  return files;
}

function headerRoot(line: string): string | null {
  const match = line.match(/^\s*\[\[?\s*([a-zA-Z0-9_-]+)(?:[.\]\s])/);
  return match?.[1] ?? null;
}

function findEntryRange(source: string, id: string): { start: number; end: number } | null {
  const lines = source.match(/.*(?:\n|$)/g) ?? [];
  let offset = 0;
  let start = -1;

  for (const line of lines) {
    const root = headerRoot(line);
    if (start === -1 && root === id && new RegExp(`^\\s*\\[\\s*${id}\\s*\\]`).test(line)) {
      start = offset;
    } else if (start !== -1 && root !== null && root !== id) {
      return { start, end: offset };
    }
    offset += line.length;
  }

  return start === -1 ? null : { start, end: source.length };
}

function upsertTomlEntry(source: string, id: string, entrySource: string): string {
  const normalizedEntry = entrySource.trimEnd() + '\n';
  const range = findEntryRange(source, id);
  if (range) {
    return source.slice(0, range.start) + normalizedEntry + source.slice(range.end);
  }

  const separator = source.length === 0
    ? ''
    : source.endsWith('\n\n') ? '' : source.endsWith('\n') ? '\n' : '\n\n';
  return source + separator + normalizedEntry;
}

function removeTomlEntry(source: string, id: string): string | null {
  const range = findEntryRange(source, id);
  if (!range) return null;
  let start = range.start;
  if (source.slice(start - 2, start) === '\n\n') {
    start -= 1;
  }
  return source.slice(0, start) + source.slice(range.end);
}

async function writeFileAtomically(filePath: string, content: string): Promise<void> {
  const tempPath = `${filePath}.${crypto.randomUUID()}.tmp`;
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(tempPath, content, 'utf-8');
  await fs.rename(tempPath, filePath);
}

router.get('/api/content/catalog', async (_req, res) => {
  try {
    const descriptors = await listContentFiles();
    const files = await Promise.all(descriptors.map(async (descriptor) => {
      try {
        const source = await fs.readFile(resolveContentPath(descriptor.path), 'utf-8');
        const parsed = parseTOML(source) as Record<string, unknown>;
        return { ...descriptor, entries: parsed };
      } catch (err) {
        return { ...descriptor, entries: {}, error: (err as Error).message };
      }
    }));
    res.set('Cache-Control', 'no-store, no-cache, must-revalidate');
    res.json({ files });
  } catch (err) {
    console.error('Failed to load content catalog:', err);
    res.status(500).json({ error: `Failed to load content: ${(err as Error).message}` });
  }
});

router.put('/api/content/entry', async (req, res) => {
  try {
    const { file, id, data } = req.body as {
      file?: string;
      id?: string;
      data?: Record<string, unknown>;
    };
    if (!file || !id || !isValidContentId(id) || !data || typeof data !== 'object') {
      return res.status(400).json({ error: 'file, a snake_case id, and data are required' });
    }

    const filePath = resolveContentPath(file);
    const entrySource = stringifyTOML({ [id]: data });
    const parsed = parseTOML(entrySource) as Record<string, unknown>;
    if (!parsed[id]) {
      return res.status(400).json({ error: 'Generated TOML did not contain the requested entry' });
    }

    let source = '';
    try {
      source = await fs.readFile(filePath, 'utf-8');
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code !== 'ENOENT') throw err;
    }

    const nextSource = upsertTomlEntry(source, id, entrySource);
    parseTOML(nextSource);
    await writeFileAtomically(filePath, nextSource);
    res.json({ success: true, file, id });
  } catch (err) {
    console.error('Failed to save content entry:', err);
    res.status(400).json({ error: `Save failed: ${(err as Error).message}` });
  }
});

router.delete('/api/content/entry', async (req, res) => {
  try {
    const file = String(req.query.file || '');
    const id = String(req.query.id || '');
    if (!file || !isValidContentId(id)) {
      return res.status(400).json({ error: 'file and a valid id are required' });
    }

    const filePath = resolveContentPath(file);
    const source = await fs.readFile(filePath, 'utf-8');
    const nextSource = removeTomlEntry(source, id);
    if (nextSource === null) {
      return res.status(404).json({ error: 'Entry not found' });
    }

    parseTOML(nextSource);
    await writeFileAtomically(filePath, nextSource);
    res.json({ success: true, file, id });
  } catch (err) {
    console.error('Failed to delete content entry:', err);
    res.status(400).json({ error: `Delete failed: ${(err as Error).message}` });
  }
});


  return router;
}
