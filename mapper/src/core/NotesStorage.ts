import type { DevNote } from '@/types';

const API_BASE = '';

class NotesStorage {
  async fetchAll(): Promise<DevNote[]> {
    const res = await fetch(`${API_BASE}/api/notes`);
    if (!res.ok) throw new Error('Failed to fetch notes');
    return res.json();
  }

  async create(note: DevNote): Promise<DevNote> {
    const res = await fetch(`${API_BASE}/api/notes`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(note),
    });
    if (!res.ok) throw new Error('Failed to create note');
    return res.json();
  }

  async update(id: string, updates: Partial<DevNote>): Promise<DevNote> {
    const res = await fetch(`${API_BASE}/api/notes/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updates),
    });
    if (!res.ok) throw new Error('Failed to update note');
    return res.json();
  }

  async remove(id: string): Promise<void> {
    const res = await fetch(`${API_BASE}/api/notes/${id}`, {
      method: 'DELETE',
    });
    if (!res.ok) throw new Error('Failed to delete note');
  }
}

export const notesStorage = new NotesStorage();
