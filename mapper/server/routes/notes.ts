import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import { Router } from 'express';

const NOTE_ID_PATTERN = /^[a-zA-Z0-9_-]{1,128}$/;
const CHUNK_QUERY_PATTERN = /^-?\d{1,7},-?\d{1,7}$/;
const CATEGORIES = new Set(['todo', 'bug', 'info', 'idea']);
const PRIORITIES = new Set(['low', 'medium', 'high']);
const STATUSES = new Set(['open', 'resolved']);
const ANCHOR_TYPES = new Set(['entity', 'mapObject', 'portal', 'wall']);

type DevNote = {
  id: string;
  x: number;
  y: number;
  chunkCoord: {
    cx: number;
    cy: number;
  };
  text: string;
  category: string;
  priority: string;
  status: string;
  anchor?: {
    type: string;
    index: number;
  };
  createdAt: string;
  updatedAt: string;
};

class NoteRequestError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCoordinate(value: unknown): value is number {
  return Number.isInteger(value) && Math.abs(value as number) <= 10_000_000;
}

function isTimestamp(value: unknown): value is string {
  return typeof value === 'string'
    && value.length <= 64
    && Number.isFinite(Date.parse(value));
}

function parseNote(value: unknown, expectedId?: string): DevNote {
  if (!isRecord(value)) {
    throw new NoteRequestError(400, 'Note must be an object');
  }

  const id = expectedId ?? value.id;
  if (typeof id !== 'string' || !NOTE_ID_PATTERN.test(id)) {
    throw new NoteRequestError(400, 'Invalid note id');
  }
  if (!isCoordinate(value.x) || !isCoordinate(value.y)) {
    throw new NoteRequestError(400, 'Note coordinates must be bounded integers');
  }
  if (
    !isRecord(value.chunkCoord)
    || !isCoordinate(value.chunkCoord.cx)
    || !isCoordinate(value.chunkCoord.cy)
  ) {
    throw new NoteRequestError(400, 'Invalid note chunk coordinate');
  }
  if (
    typeof value.text !== 'string'
    || value.text.trim().length === 0
    || value.text.length > 5_000
  ) {
    throw new NoteRequestError(400, 'Note text must contain 1-5000 characters');
  }
  if (typeof value.category !== 'string' || !CATEGORIES.has(value.category)) {
    throw new NoteRequestError(400, 'Invalid note category');
  }
  if (typeof value.priority !== 'string' || !PRIORITIES.has(value.priority)) {
    throw new NoteRequestError(400, 'Invalid note priority');
  }
  if (typeof value.status !== 'string' || !STATUSES.has(value.status)) {
    throw new NoteRequestError(400, 'Invalid note status');
  }
  if (!isTimestamp(value.createdAt) || !isTimestamp(value.updatedAt)) {
    throw new NoteRequestError(400, 'Invalid note timestamp');
  }

  let anchor: DevNote['anchor'];
  if (value.anchor !== undefined) {
    if (
      !isRecord(value.anchor)
      || typeof value.anchor.type !== 'string'
      || !ANCHOR_TYPES.has(value.anchor.type)
      || !Number.isInteger(value.anchor.index)
      || (value.anchor.index as number) < 0
    ) {
      throw new NoteRequestError(400, 'Invalid note anchor');
    }
    anchor = {
      type: value.anchor.type,
      index: value.anchor.index as number,
    };
  }

  return {
    id,
    x: value.x,
    y: value.y,
    chunkCoord: {
      cx: value.chunkCoord.cx,
      cy: value.chunkCoord.cy,
    },
    text: value.text.trim(),
    category: value.category,
    priority: value.priority,
    status: value.status,
    ...(anchor ? { anchor } : {}),
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  };
}

async function loadNotesFromDisk(notesFile: string): Promise<DevNote[]> {
  try {
    const data = await fs.readFile(notesFile, 'utf-8');
    const parsed: unknown = JSON.parse(data);
    if (!Array.isArray(parsed)) {
      throw new Error('notes file must contain a JSON array');
    }
    const notes = parsed.map((value, index) => {
      try {
        return parseNote(value);
      } catch (error) {
        throw new Error(`invalid note at index ${index}: ${(error as Error).message}`);
      }
    });
    if (new Set(notes.map((note) => note.id)).size !== notes.length) {
      throw new Error('notes file contains duplicate ids');
    }
    return notes;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }
}

async function saveNotesToDisk(notesFile: string, notes: readonly DevNote[]): Promise<void> {
  const tempPath = `${notesFile}.${crypto.randomUUID()}.tmp`;
  await fs.mkdir(path.dirname(notesFile), { recursive: true });
  try {
    await fs.writeFile(tempPath, JSON.stringify(notes, null, 2), 'utf-8');
    await fs.rename(tempPath, notesFile);
  } catch (error) {
    await fs.rm(tempPath, { force: true });
    throw error;
  }
}

export async function createNotesRouter(
  notesFile: string,
): Promise<{ router: Router; count: number }> {
  const router = Router();
  let notesCache = await loadNotesFromDisk(notesFile);
  let mutationQueue = Promise.resolve();

  async function mutateNotes<T>(
    mutation: (current: readonly DevNote[]) => { notes: DevNote[]; result: T },
  ): Promise<T> {
    const operation = mutationQueue.then(async () => {
      const { notes, result } = mutation(notesCache);
      await saveNotesToDisk(notesFile, notes);
      notesCache = notes;
      return result;
    });
    mutationQueue = operation.then(
      () => undefined,
      () => undefined,
    );
    return operation;
  }

  router.param('id', (_req, res, next, value) => {
    if (!NOTE_ID_PATTERN.test(value)) {
      return res.status(400).json({ error: 'Invalid note id' });
    }
    return next();
  });

  router.get('/api/notes', (req, res) => {
    let filtered = notesCache;
    const { status, category, priority, chunk } = req.query;
    if (typeof status === 'string') filtered = filtered.filter((note) => note.status === status);
    if (typeof category === 'string') {
      filtered = filtered.filter((note) => note.category === category);
    }
    if (typeof priority === 'string') {
      filtered = filtered.filter((note) => note.priority === priority);
    }
    if (chunk !== undefined) {
      if (typeof chunk !== 'string' || !CHUNK_QUERY_PATTERN.test(chunk)) {
        return res.status(400).json({ error: 'Invalid chunk filter' });
      }
      const [cx, cy] = chunk.split(',').map(Number);
      filtered = filtered.filter(
        (note) => note.chunkCoord.cx === cx && note.chunkCoord.cy === cy,
      );
    }
    return res.json(filtered);
  });

  router.get('/api/notes/:id', (req, res) => {
    const note = notesCache.find((entry) => entry.id === req.params.id);
    if (!note) return res.status(404).json({ error: 'Note not found' });
    return res.json(note);
  });

  router.post('/api/notes', async (req, res) => {
    try {
      const note = parseNote(req.body);
      const created = await mutateNotes((current) => {
        if (current.some((entry) => entry.id === note.id)) {
          throw new NoteRequestError(409, 'Note id already exists');
        }
        return { notes: [...current, note], result: note };
      });
      return res.status(201).json(created);
    } catch (error) {
      if (error instanceof NoteRequestError) {
        return res.status(error.status).json({ error: error.message });
      }
      console.error('Error creating note:', error);
      return res.status(500).json({ error: 'Failed to create note' });
    }
  });

  router.put('/api/notes/:id', async (req, res) => {
    try {
      const updated = await mutateNotes((current) => {
        const index = current.findIndex((entry) => entry.id === req.params.id);
        if (index === -1) throw new NoteRequestError(404, 'Note not found');
        const note = parseNote({ ...current[index], ...req.body }, req.params.id);
        const notes = [...current];
        notes[index] = note;
        return { notes, result: note };
      });
      return res.json(updated);
    } catch (error) {
      if (error instanceof NoteRequestError) {
        return res.status(error.status).json({ error: error.message });
      }
      console.error('Error updating note:', error);
      return res.status(500).json({ error: 'Failed to update note' });
    }
  });

  router.delete('/api/notes/:id', async (req, res) => {
    try {
      await mutateNotes((current) => ({
        notes: current.filter((entry) => entry.id !== req.params.id),
        result: undefined,
      }));
      return res.json({ success: true });
    } catch (error) {
      console.error('Error deleting note:', error);
      return res.status(500).json({ error: 'Failed to delete note' });
    }
  });

  return { router, count: notesCache.length };
}
