import { useState, useRef, useCallback, useEffect } from 'react';
import { useEditorStore } from '@/state/store';
import { objectLoader } from '@/core/ObjectLoader';
import { tilesetLoader } from '@/core/TilesetLoader';
import styles from './AssetManager.module.css';

interface QueueItem {
  file: File;
  preview: string; // data URL
  width: number;
  height: number;
  name: string;
  id?: number;
  animation: { frames: number; fps: number } | null;
  detecting: boolean;
}

type Tab = 'objects' | 'walls' | 'tiles';

export function AssetManager() {
  const { assetManagerOpen, assetManagerTab, closeAssetManager, refreshAssets } = useEditorStore();
  const [tab, setTab] = useState<Tab>(assetManagerTab);
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState<{ text: string; type: 'info' | 'success' | 'error' } | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Sync tab from store when opened
  useEffect(() => {
    if (assetManagerOpen) {
      setTab(assetManagerTab);
      setQueue([]);
      setStatus(null);
      setProgress(0);
    }
  }, [assetManagerOpen, assetManagerTab]);

  const processFiles = useCallback(async (files: FileList | File[]) => {
    const newItems: QueueItem[] = [];

    // Fetch next available ID for sequential naming (objects/walls only)
    let nextId: number | null = null;
    if (tab !== 'tiles') {
      try {
        const resp = await fetch(`/mapper/api/assets/next-id/${tab}`);
        if (resp.ok) {
          const data = await resp.json();
          nextId = data.nextId;
        }
      } catch {
        // Fall back to filename-based naming
      }
    }

    for (const file of Array.from(files)) {
      if (!file.type.startsWith('image/png')) continue;

      // Read image for preview and dimensions
      const preview = await readFileAsDataURL(file);
      const dims = await getImageDimensions(preview);

      // Auto-assign sequential ID and name for objects/walls
      const autoId = nextId !== null ? nextId + newItems.length : undefined;
      const autoName = autoId !== undefined ? String(autoId) : file.name.replace(/\.png$/i, '');

      const item: QueueItem = {
        file,
        preview,
        width: dims.width,
        height: dims.height,
        name: autoName,
        id: autoId,
        animation: null,
        detecting: false,
      };

      newItems.push(item);
    }

    setQueue(prev => [...prev, ...newItems]);

    // Auto-detect animations for non-tile items
    if (tab !== 'tiles') {
      for (const item of newItems) {
        detectAnimation(item);
      }
    }
  }, [tab]);

  const detectAnimation = async (item: QueueItem) => {
    setQueue(prev => prev.map(q => q === item ? { ...q, detecting: true } : q));

    try {
      const formData = new FormData();
      formData.append('file', item.file);
      const resp = await fetch('/mapper/api/assets/detect-animation', { method: 'POST', body: formData });
      const result = await resp.json();

      setQueue(prev => prev.map(q =>
        q === item ? { ...q, animation: result, detecting: false } : q
      ));
    } catch {
      setQueue(prev => prev.map(q => q === item ? { ...q, detecting: false } : q));
    }
  };

  const removeFromQueue = (index: number) => {
    setQueue(prev => prev.filter((_, i) => i !== index));
  };

  const updateQueueItem = (index: number, updates: Partial<QueueItem>) => {
    setQueue(prev => prev.map((item, i) => i === index ? { ...item, ...updates } : item));
  };

  const handleImport = async () => {
    if (queue.length === 0) return;
    setImporting(true);
    setProgress(0);
    setStatus({ text: 'Uploading...', type: 'info' });

    try {
      const results: Array<{ id: number; name: string; width: number; height: number; animation?: { frames: number; fps: number } | null }> = [];

      for (let i = 0; i < queue.length; i++) {
        const item = queue[i];
        setProgress(((i) / queue.length) * 100);

        const formData = new FormData();
        formData.append('file', item.file);
        formData.append('category', tab);
        if (item.name) formData.append('name', item.name);
        if (item.id !== undefined) formData.append('id', String(item.id));
        if (item.animation) formData.append('animation', JSON.stringify(item.animation));

        const resp = await fetch('/mapper/api/assets/upload', { method: 'POST', body: formData });
        if (!resp.ok) {
          const err = await resp.json();
          throw new Error(err.error || 'Upload failed');
        }

        const result = await resp.json();
        results.push(result);

        // Optimistic update: add to loaders immediately
        if (tab === 'objects') {
          await objectLoader.addObject(
            { id: result.id, name: result.name || String(result.id), width: result.width, height: result.height },
            `/mapper/assets/sprites/objects/${result.id}.png`,
            result.animation || undefined
          );
        } else if (tab === 'walls') {
          await objectLoader.addWall(
            { id: result.id, name: result.name || String(result.id), width: result.width, height: result.height },
            `/mapper/assets/sprites/walls/${result.id}.png`,
            result.animation || undefined
          );
        } else if (tab === 'tiles' && result.tileIds) {
          // Load each new tile image and add to tileset
          const tileImages: HTMLImageElement[] = [];
          for (const tileId of result.tileIds) {
            const img = new Image();
            await new Promise<void>((resolve, reject) => {
              img.onload = () => resolve();
              img.onerror = () => reject(new Error(`Failed to load tile ${tileId}`));
              img.src = `/mapper/assets/sprites/tiles_preview/tile_${tileId}.png`;
            });
            tileImages.push(img);
          }
          tilesetLoader.addTiles(tileImages);
        }
      }

      setProgress(100);
      refreshAssets();

      const count = tab === 'tiles'
        ? results.reduce((sum, r) => sum + ((r as any).count || 1), 0)
        : results.length;
      setStatus({ text: `Imported ${count} ${tab} successfully!`, type: 'success' });
      setQueue([]);

      // After a delay, reload from rebuilt atlas (background rebuild should be done)
      setTimeout(async () => {
        try {
          if (tab === 'tiles') {
            await tilesetLoader.reloadTileset();
          } else {
            await objectLoader.reloadFromConfig();
          }
          refreshAssets();
        } catch (err) {
          console.warn('Atlas reload failed (may still be building):', err);
        }
      }, 5000);

    } catch (err) {
      setStatus({ text: `Error: ${(err as Error).message}`, type: 'error' });
    } finally {
      setImporting(false);
    }
  };

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragActive(false);
    if (e.dataTransfer.files.length > 0) {
      processFiles(e.dataTransfer.files);
    }
  }, [processFiles]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragActive(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDragActive(false);
  }, []);

  if (!assetManagerOpen) return null;

  return (
    <div className={styles.overlay} onClick={closeAssetManager}>
      <div className={styles.modal} onClick={e => e.stopPropagation()}>
        <div className={styles.header}>
          <h3>Import Assets</h3>
          <button className={styles.closeButton} onClick={closeAssetManager}>&times;</button>
        </div>

        <div className={styles.tabs}>
          {(['objects', 'walls', 'tiles'] as Tab[]).map(t => (
            <button
              key={t}
              className={`${styles.tab} ${tab === t ? styles.activeTab : ''}`}
              onClick={() => { setTab(t); setQueue([]); setStatus(null); }}
            >
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>

        <div
          className={`${styles.dropZone} ${dragActive ? styles.dropZoneActive : ''}`}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onClick={() => fileInputRef.current?.click()}
        >
          <input
            ref={fileInputRef}
            type="file"
            accept=".png"
            multiple
            style={{ display: 'none' }}
            onChange={e => e.target.files && processFiles(e.target.files)}
          />
          <div className={styles.dropZoneText}>
            <strong>Drop PNG files here</strong> or click to browse
            {tab === 'tiles' && <div style={{ marginTop: 4, fontSize: 11, color: '#666' }}>Tiles must be 64x32px (or strips of 64px-wide tiles)</div>}
          </div>
        </div>

        {importing && (
          <div className={styles.progressBar}>
            <div className={styles.progressFill} style={{ width: `${progress}%` }} />
          </div>
        )}

        <div className={styles.queue}>
          {queue.map((item, i) => (
            <div key={i} className={styles.queueItem}>
              <img src={item.preview} className={styles.thumbnail} alt="" />
              <div className={styles.itemInfo}>
                <div className={styles.itemName}>{item.file.name}</div>
                <div className={styles.itemMeta}>
                  {item.width}x{item.height}px
                  {item.detecting && ' — detecting animation...'}
                </div>
              </div>
              <div className={styles.itemFields}>
                {tab !== 'tiles' && (
                  <input
                    className={styles.smallInput}
                    placeholder="Name"
                    value={item.name}
                    onChange={e => updateQueueItem(i, { name: e.target.value })}
                  />
                )}
                {item.animation && (
                  <span className={styles.animBadge}>
                    {item.animation.frames}f @ {item.animation.fps}fps
                  </span>
                )}
              </div>
              <button className={styles.removeButton} onClick={() => removeFromQueue(i)}>&times;</button>
            </div>
          ))}
        </div>

        <div className={styles.actions}>
          <div className={styles.status}>
            {status && (
              <span className={status.type === 'success' ? styles.statusSuccess : status.type === 'error' ? styles.statusError : ''}>
                {status.text}
              </span>
            )}
          </div>
          <button
            className={styles.importButton}
            onClick={handleImport}
            disabled={queue.length === 0 || importing}
          >
            {importing ? 'Importing...' : `Import ${queue.length} file${queue.length !== 1 ? 's' : ''}`}
          </button>
        </div>
      </div>
    </div>
  );
}

function readFileAsDataURL(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

function getImageDimensions(src: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ width: img.width, height: img.height });
    img.onerror = reject;
    img.src = src;
  });
}
