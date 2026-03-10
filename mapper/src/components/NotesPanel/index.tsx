import { useState, useMemo, useEffect } from 'react';
import { useEditorStore } from '@/state/store';
import { notesStorage } from '@/core/NotesStorage';
import { screenToWorldTile } from '@/core/coords';
import type { DevNote, NoteCategory, NotePriority, NoteStatus } from '@/types';
import styles from './NotesPanel.module.css';

const CATEGORY_COLORS: Record<NoteCategory, string> = {
  todo: '#ff9800',
  bug: '#f44336',
  info: '#2196f3',
  idea: '#4caf50',
};

function generateId(): string {
  return crypto.randomUUID();
}

export function NotesPanel() {
  const {
    notes,
    showNotes,
    selectedNoteId,
    notesPanelCollapsed,
    hoveredTile,
    viewport,
    addNote,
    updateNote,
    removeNote,
    setSelectedNoteId,
    setShowNotes,
    setNotesPanelCollapsed,
    setViewport,
  } = useEditorStore();

  const [filterCategory, setFilterCategory] = useState<NoteCategory | null>(null);
  const [filterStatus, setFilterStatus] = useState<NoteStatus | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);

  // Edit form state
  const [editText, setEditText] = useState('');
  const [editCategory, setEditCategory] = useState<NoteCategory>('todo');
  const [editPriority, setEditPriority] = useState<NotePriority>('medium');
  const [editStatus, setEditStatus] = useState<NoteStatus>('open');

  const filteredNotes = useMemo(() => {
    let result = notes;
    if (filterCategory) result = result.filter(n => n.category === filterCategory);
    if (filterStatus) result = result.filter(n => n.status === filterStatus);
    return result.sort((a, b) => {
      // Open before resolved
      if (a.status !== b.status) return a.status === 'open' ? -1 : 1;
      // High priority first
      const priOrder = { high: 0, medium: 1, low: 2 };
      return priOrder[a.priority] - priOrder[b.priority];
    });
  }, [notes, filterCategory, filterStatus]);

  // Auto-open editing when a new note is selected with empty text (e.g. from right-click)
  useEffect(() => {
    if (selectedNoteId && !editingId) {
      const note = notes.find(n => n.id === selectedNoteId);
      if (note && note.text === '') {
        startEditing(note);
      }
    }
  }, [selectedNoteId]);

  const startEditing = (note: DevNote) => {
    setEditingId(note.id);
    setEditText(note.text);
    setEditCategory(note.category);
    setEditPriority(note.priority);
    setEditStatus(note.status);
  };

  const handleCreate = async () => {
    // Use hovered tile, or center of current viewport
    const centerTile = screenToWorldTile(
      { sx: window.innerWidth / 2, sy: window.innerHeight / 2 },
      viewport
    );
    const tile = hoveredTile || centerTile;
    const CHUNK_SIZE = 32;
    const now = new Date().toISOString();
    const note: DevNote = {
      id: generateId(),
      x: tile.wx,
      y: tile.wy,
      chunkCoord: {
        cx: Math.floor(tile.wx / CHUNK_SIZE),
        cy: Math.floor(tile.wy / CHUNK_SIZE),
      },
      text: '',
      category: 'todo',
      priority: 'medium',
      status: 'open',
      createdAt: now,
      updatedAt: now,
    };

    addNote(note);
    notesStorage.create(note).catch(console.error);
    setSelectedNoteId(note.id);
    startEditing(note);
  };

  const handleSave = async () => {
    if (!editingId) return;
    const updates: Partial<DevNote> = {
      text: editText,
      category: editCategory,
      priority: editPriority,
      status: editStatus,
      updatedAt: new Date().toISOString(),
    };
    updateNote(editingId, updates);
    notesStorage.update(editingId, updates).catch(console.error);
    setEditingId(null);
  };

  const handleDelete = async (id: string) => {
    removeNote(id);
    notesStorage.remove(id).catch(console.error);
    if (editingId === id) setEditingId(null);
  };

  const handleToggleResolve = async (note: DevNote) => {
    const newStatus: NoteStatus = note.status === 'open' ? 'resolved' : 'open';
    const updates = { status: newStatus, updatedAt: new Date().toISOString() };
    updateNote(note.id, updates);
    notesStorage.update(note.id, updates).catch(console.error);
  };

  const flyToNote = (note: DevNote) => {
    setSelectedNoteId(note.id);
    // Center viewport on note's world position
    const TILE_WIDTH = 64;
    const TILE_HEIGHT = 32;
    const screenX = (note.x - note.y) * (TILE_WIDTH / 2) * viewport.zoom;
    const screenY = (note.x + note.y) * (TILE_HEIGHT / 2) * viewport.zoom;
    const canvas = document.querySelector('canvas');
    if (canvas) {
      setViewport({
        offsetX: canvas.width / 2 - screenX,
        offsetY: canvas.height / 2 - screenY,
      });
    }
  };

  const openCount = notes.filter(n => n.status === 'open').length;

  return (
    <div className={styles.panel}>
      <div className={styles.header} onClick={() => setNotesPanelCollapsed(!notesPanelCollapsed)}>
        <div className={styles.headerLeft}>
          <span className={`${styles.arrow} ${notesPanelCollapsed ? styles.arrowCollapsed : ''}`}>&#x25BC;</span>
          <span className={styles.title}>Notes</span>
          {openCount > 0 && <span className={styles.badge}>{openCount}</span>}
        </div>
        <div className={styles.headerActions} onClick={(e) => e.stopPropagation()}>
          <button
            className={styles.iconButton}
            onClick={() => setShowNotes(!showNotes)}
            title={showNotes ? 'Hide pins' : 'Show pins'}
          >
            {showNotes ? '\u{1F441}' : '\u{1F441}\u200D\u{1F5E8}'}
          </button>
          <button className={styles.iconButton} onClick={handleCreate} title="Add note at cursor">
            +
          </button>
        </div>
      </div>

      {!notesPanelCollapsed && (
        <div className={styles.content}>
          <div className={styles.filters}>
            {(['todo', 'bug', 'info', 'idea'] as NoteCategory[]).map(cat => (
              <button
                key={cat}
                className={`${styles.filterChip} ${filterCategory === cat ? styles.filterChipActive : ''}`}
                onClick={() => setFilterCategory(filterCategory === cat ? null : cat)}
                style={filterCategory === cat ? { borderColor: CATEGORY_COLORS[cat] } : undefined}
              >
                {cat}
              </button>
            ))}
            <button
              className={`${styles.filterChip} ${filterStatus === 'open' ? styles.filterChipActive : ''}`}
              onClick={() => setFilterStatus(filterStatus === 'open' ? null : 'open')}
            >
              open
            </button>
            <button
              className={`${styles.filterChip} ${filterStatus === 'resolved' ? styles.filterChipActive : ''}`}
              onClick={() => setFilterStatus(filterStatus === 'resolved' ? null : 'resolved')}
            >
              resolved
            </button>
          </div>

          <div className={styles.noteList}>
            {filteredNotes.length === 0 && (
              <div className={styles.emptyState}>No notes yet. Click + to add one.</div>
            )}
            {filteredNotes.map(note => (
              editingId === note.id ? (
                <div key={note.id} className={styles.editForm}>
                  <textarea
                    className={styles.textarea}
                    value={editText}
                    onChange={(e) => setEditText(e.target.value)}
                    placeholder="Note text..."
                    autoFocus
                  />
                  <div className={styles.fieldRow}>
                    <select
                      className={styles.select}
                      value={editCategory}
                      onChange={(e) => setEditCategory(e.target.value as NoteCategory)}
                    >
                      <option value="todo">TODO</option>
                      <option value="bug">Bug</option>
                      <option value="info">Info</option>
                      <option value="idea">Idea</option>
                    </select>
                    <select
                      className={styles.select}
                      value={editPriority}
                      onChange={(e) => setEditPriority(e.target.value as NotePriority)}
                    >
                      <option value="low">Low</option>
                      <option value="medium">Medium</option>
                      <option value="high">High</option>
                    </select>
                    <select
                      className={styles.select}
                      value={editStatus}
                      onChange={(e) => setEditStatus(e.target.value as NoteStatus)}
                    >
                      <option value="open">Open</option>
                      <option value="resolved">Resolved</option>
                    </select>
                  </div>
                  <div className={styles.noteCoord}>
                    Tile ({note.x}, {note.y}) &mdash; Chunk ({note.chunkCoord.cx}, {note.chunkCoord.cy})
                  </div>
                  <div className={styles.formActions}>
                    <button className={styles.deleteButton} onClick={() => handleDelete(note.id)}>Delete</button>
                    <button className={styles.saveButton} onClick={handleSave}>Save</button>
                  </div>
                </div>
              ) : (
                <div
                  key={note.id}
                  className={`${styles.noteCard} ${selectedNoteId === note.id ? styles.noteCardSelected : ''} ${note.status === 'resolved' ? styles.noteCardResolved : ''}`}
                  onClick={() => flyToNote(note)}
                  onDoubleClick={() => startEditing(note)}
                >
                  <div className={styles.noteHeader}>
                    <span
                      className={styles.categoryDot}
                      style={{ background: CATEGORY_COLORS[note.category] }}
                    />
                    <span style={{ flex: 1, color: '#aaa' }}>{note.category}</span>
                    {note.priority === 'high' && (
                      <span className={`${styles.priorityBadge} ${styles.priorityHigh}`}>high</span>
                    )}
                    {note.priority === 'low' && (
                      <span className={styles.priorityBadge}>low</span>
                    )}
                    <button
                      className={styles.iconButton}
                      onClick={(e) => { e.stopPropagation(); handleToggleResolve(note); }}
                      title={note.status === 'open' ? 'Resolve' : 'Reopen'}
                    >
                      {note.status === 'open' ? '\u2713' : '\u21A9'}
                    </button>
                  </div>
                  {note.text && <div className={styles.noteText}>{note.text}</div>}
                  <div className={styles.noteCoord}>
                    ({note.x}, {note.y})
                    {note.anchor && <span className={styles.anchorBadge}> &middot; {note.anchor.type}</span>}
                  </div>
                </div>
              )
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
