import { useRef, useEffect, useState } from 'react';
import { useEditorStore } from '@/state/store';
import { objectLoader } from '@/core/ObjectLoader';
import type { ObjectDefinition } from '@/types';
import styles from './ObjectPalette.module.css';

type Category = 'objects' | 'walls';
type WallTool = 'wallDown' | 'wallRight';

export function ObjectPalette() {
  const { selectedObjectId, setSelectedObjectId, setActiveTool, activeTool, openAssetManager, assetManagerOpen } = useEditorStore();
  const [objects, setObjects] = useState<ObjectDefinition[]>([]);
  const [walls, setWalls] = useState<ObjectDefinition[]>([]);
  const [category, setCategory] = useState<Category>('objects');
  const [filter, setFilter] = useState('');
  const [lastWallTool, setLastWallTool] = useState<WallTool>('wallDown');
  const canvasRefs = useRef<Map<number, HTMLCanvasElement>>(new Map());

  // Load objects and walls when component mounts
  useEffect(() => {
    const loadedObjects = objectLoader.getObjectsWithImages();
    const loadedWalls = objectLoader.getWallsWithImages();
    setObjects(loadedObjects);
    setWalls(loadedWalls);
  }, []);

  // Track when wall tools are used so we remember the last one
  useEffect(() => {
    if (activeTool === 'wallDown' || activeTool === 'wallRight') {
      setLastWallTool(activeTool as WallTool);
    }
  }, [activeTool]);

  // Get current items based on category
  const currentItems = category === 'objects' ? objects : walls;

  // Draw object previews (static once, animated via rAF)
  useEffect(() => {
    const drawPreview = (obj: ObjectDefinition, frameIndex?: number) => {
      const canvas = canvasRefs.current.get(obj.id);
      if (!canvas || !obj.image) return;

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const maxSize = 64;
      const scale = Math.min(maxSize / obj.width, maxSize / obj.height, 1);
      const drawWidth = obj.width * scale;
      const drawHeight = obj.height * scale;

      canvas.width = maxSize;
      canvas.height = maxSize;

      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, maxSize, maxSize);
      const r = obj.atlasRect;
      if (r) {
        const srcX = (frameIndex !== undefined) ? r.x + frameIndex * r.w : r.x;
        ctx.drawImage(
          obj.image,
          srcX, r.y, r.w, r.h,
          (maxSize - drawWidth) / 2,
          maxSize - drawHeight,
          drawWidth,
          drawHeight
        );
      } else {
        ctx.drawImage(
          obj.image,
          (maxSize - drawWidth) / 2,
          maxSize - drawHeight,
          drawWidth,
          drawHeight
        );
      }
    };

    // Draw all static sprites once
    for (const obj of currentItems) {
      if (!obj.frames || obj.frames <= 1) {
        drawPreview(obj);
      }
    }

    // Animate sprites with frames > 1
    const animatedItems = currentItems.filter(obj => obj.frames && obj.frames > 1);
    if (animatedItems.length === 0) return;

    let rafId: number;
    const animate = () => {
      const now = performance.now() / 1000;
      for (const obj of animatedItems) {
        const fps = obj.fps ?? 4;
        const frameIndex = Math.floor(now * fps) % obj.frames!;
        drawPreview(obj, frameIndex);
      }
      rafId = requestAnimationFrame(animate);
    };
    rafId = requestAnimationFrame(animate);

    return () => cancelAnimationFrame(rafId);
  }, [currentItems]);

  const filteredObjects = filter
    ? currentItems.filter((obj) => obj.name.toLowerCase().includes(filter.toLowerCase()))
    : currentItems;

  const setCanvasRef = (id: number, el: HTMLCanvasElement | null) => {
    if (el) {
      canvasRefs.current.set(id, el);
    } else {
      canvasRefs.current.delete(id);
    }
  };

  // Refresh when asset manager closes (new assets may have been imported)
  useEffect(() => {
    if (!assetManagerOpen) {
      setObjects(objectLoader.getObjectsWithImages());
      setWalls(objectLoader.getWallsWithImages());
    }
  }, [assetManagerOpen]);

  return (
    <div className={styles.palette}>
      <div className={styles.header}>
        <div className={styles.title}>Objects & Walls</div>
        <button
          className={styles.addButton}
          onClick={() => openAssetManager(category)}
          title="Import assets..."
        >+</button>
      </div>
      <div className={styles.tabs}>
        <button
          className={`${styles.tab} ${category === 'objects' ? styles.activeTab : ''}`}
          onClick={() => setCategory('objects')}
        >
          Objects ({objects.length})
        </button>
        <button
          className={`${styles.tab} ${category === 'walls' ? styles.activeTab : ''}`}
          onClick={() => setCategory('walls')}
        >
          Walls ({walls.length})
        </button>
      </div>
      <input
        type="text"
        className={styles.search}
        placeholder="Search objects..."
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      />
      <div className={styles.info}>
        {selectedObjectId ? `Selected: ${(objects.find((o) => o.id === selectedObjectId) || walls.find((o) => o.id === selectedObjectId))?.name || selectedObjectId}` : 'None selected'}
      </div>
      <div className={styles.grid}>
        {filteredObjects.map((obj) => (
          <button
            key={obj.id}
            className={`${styles.item} ${selectedObjectId === obj.id ? styles.selected : ''}`}
            onClick={() => {
              setSelectedObjectId(obj.id);
              // Use wall tool when selecting from walls tab, object tool otherwise
              setActiveTool(category === 'walls' ? lastWallTool : 'object');
            }}
            title={`${obj.name} (${obj.width}x${obj.height})`}
          >
            <canvas
              ref={(el) => setCanvasRef(obj.id, el)}
              className={styles.preview}
              width={64}
              height={64}
            />
            <span className={styles.name}>{obj.name}</span>
          </button>
        ))}
        {filteredObjects.length === 0 && (
          <div className={styles.empty}>No objects found</div>
        )}
      </div>
    </div>
  );
}
