import { Router } from 'express';
import fs from 'node:fs/promises';
import path from 'node:path';

type DevNote = {
  id: string;
  text: string;
  category?: string;
  priority?: string;
  status?: string;
  chunkCoord?: {
    cx: number;
    cy: number;
  };
  [key: string]: unknown;
};

// In-memory notes cache


async function loadNotesFromDisk(notesFile: string): Promise<DevNote[]> {
  try {
    const data = await fs.readFile(notesFile, 'utf-8');
    return JSON.parse(data);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw err;
  }
}

async function saveNotesToDisk(notesFile: string, notes: DevNote[]): Promise<void> {
  await fs.mkdir(path.dirname(notesFile), { recursive: true });
  await fs.writeFile(notesFile, JSON.stringify(notes, null, 2));
}



export async function createNotesRouter(notesFile: string): Promise<{ router: Router; count: number }> {
  const router = Router();
  let notesCache = await loadNotesFromDisk(notesFile);

  router.param('id', (req, res, next, value) => {
    if (!/^[a-zA-Z0-9_-]+$/.test(value)) {
      return res.status(400).json({ error: 'Invalid note id' });
    }
    return next();
  });

// --- Notes API ---

// Get all notes (with optional filters)
router.get('/api/notes', (_req, res) => {
  let filtered = notesCache;

  const { status, category, priority, chunk } = _req.query;
  if (status) filtered = filtered.filter(n => n.status === status);
  if (category) filtered = filtered.filter(n => n.category === category);
  if (priority) filtered = filtered.filter(n => n.priority === priority);
  if (chunk) {
    const [cx, cy] = (chunk as string).split(',').map(Number);
    filtered = filtered.filter(n => n.chunkCoord?.cx === cx && n.chunkCoord?.cy === cy);
  }

  res.json(filtered);
});

// Get single note
router.get('/api/notes/:id', (req, res) => {
  const note = notesCache.find(n => n.id === req.params.id);
  if (!note) return res.status(404).json({ error: 'Note not found' });
  res.json(note);
});

// Create note
router.post('/api/notes', async (req, res) => {
  try {
    const note = req.body;
    if (!note.id || note.text === undefined) {
      return res.status(400).json({ error: 'Note must have id and text' });
    }
    notesCache.push(note);
    await saveNotesToDisk(notesFile, notesCache);
    res.json(note);
  } catch (err) {
    console.error('Error creating note:', err);
    res.status(500).json({ error: 'Failed to create note' });
  }
});

// Update note
router.put('/api/notes/:id', async (req, res) => {
  try {
    const idx = notesCache.findIndex(n => n.id === req.params.id);
    if (idx === -1) return res.status(404).json({ error: 'Note not found' });
    notesCache[idx] = { ...notesCache[idx], ...req.body, id: req.params.id };
    await saveNotesToDisk(notesFile, notesCache);
    res.json(notesCache[idx]);
  } catch (err) {
    console.error('Error updating note:', err);
    res.status(500).json({ error: 'Failed to update note' });
  }
});

// Delete note
router.delete('/api/notes/:id', async (req, res) => {
  try {
    const idx = notesCache.findIndex(n => n.id === req.params.id);
    if (idx === -1) return res.json({ success: true }); // idempotent
    notesCache.splice(idx, 1);
    await saveNotesToDisk(notesFile, notesCache);
    res.json({ success: true });
  } catch (err) {
    console.error('Error deleting note:', err);
    res.status(500).json({ error: 'Failed to delete note' });
  }
});


  return { router, count: notesCache.length };
}
