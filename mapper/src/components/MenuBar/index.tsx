import { useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import JSZip from 'jszip';
import { useEditorStore, cancelPendingSave } from '@/state/store';
import { chunkManager } from '@/core/ChunkManager';
import { history } from '@/core/History';
import { storage } from '@/core/Storage';
import { interiorStorage } from '@/core/InteriorStorage';
import styles from './MenuBar.module.css';

export function MenuBar({ onOpenContentStudio }: { onOpenContentStudio: () => void }) {
  const [showNewInteriorModal, setShowNewInteriorModal] = useState(false);
  const [showOpenInteriorModal, setShowOpenInteriorModal] = useState(false);
  const [showResizeInteriorModal, setShowResizeInteriorModal] = useState(false);
  const [showDownloadChunkModal, setShowDownloadChunkModal] = useState(false);
  const [downloadChunkCoord, setDownloadChunkCoord] = useState('0, 0');
  const [downloadInteriorId, setDownloadInteriorId] = useState('');
  const [newInteriorId, setNewInteriorId] = useState('');
  const [newInteriorName, setNewInteriorName] = useState('');
  const [newInteriorWidth, setNewInteriorWidth] = useState(16);
  const [newInteriorHeight, setNewInteriorHeight] = useState(16);
  const [resizeWidth, setResizeWidth] = useState(16);
  const [resizeHeight, setResizeHeight] = useState(16);

  const {
    chunks,
    showGrid,
    showChunkBounds,
    toggleGrid,
    toggleChunkBounds,
    viewport,
    setViewport,
    getDirtyChunks,
    markAllClean,
    setChunks,
    setWorldBounds,
    isConnected,
    editorMode,
    currentInterior,
    currentInteriorId,
    availableInteriors,
    switchToOverworld,
    createInterior,
    loadInterior,
    saveCurrentInterior,
    setAvailableInteriors,
    resizeInterior,
    openAssetManager,
    currentWorld,
    availableWorlds,
    switchWorld,
    paletteSide,
    togglePaletteSide,
  } = useEditorStore();

  const [rebuildingAtlas, setRebuildingAtlas] = useState(false);

  const importInputRef = useRef<HTMLInputElement>(null);
  const importFullInputRef = useRef<HTMLInputElement>(null);
  const importChunkInputRef = useRef<HTMLInputElement>(null);

  // Store the directory handle for reuse
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const directoryHandleRef = useRef<any>(null);

  const handleSaveAll = async () => {
    const dirtyChunks = getDirtyChunks();
    if (dirtyChunks.length === 0) {
      alert('No changes to save');
      return;
    }

    // Export all dirty chunks from the store (not ChunkManager's cache)
    const exports: { filename: string; content: string }[] = [];
    for (const chunk of dirtyChunks) {
      const json = chunkManager.exportChunkDataToJSON(chunk);
      exports.push({
        filename: `chunk_${chunk.coord.cx}_${chunk.coord.cy}.json`,
        content: json,
      });
    }

    // Download as individual files (in a real app, this would write to the server)
    for (const exp of exports) {
      const blob = new Blob([exp.content], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = exp.filename;
      a.click();
      URL.revokeObjectURL(url);
    }

    markAllClean();
    alert(`Saved ${exports.length} chunk(s)`);
  };

  // Export all chunks to a directory (for testing in live game)
  const handleExportToServer = async () => {
    try {
      // Check if File System Access API is supported
      if ('showDirectoryPicker' in window) {
        await exportToDirectory();
      } else {
        // Fallback: download as ZIP
        await exportAsZip();
      }
    } catch (err) {
      if ((err as Error).name === 'AbortError') {
        // User cancelled
        return;
      }
      console.error('Export failed:', err);
      alert(`Export failed: ${(err as Error).message}`);
    }
  };

  // Export using File System Access API (Chrome/Edge)
  const exportToDirectory = async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let dirHandle: any = directoryHandleRef.current;

    if (!dirHandle) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      dirHandle = await (window as any).showDirectoryPicker({
        mode: 'readwrite',
      });
      directoryHandleRef.current = dirHandle;
    }

    // Verify we still have permission
    const permission = await dirHandle.queryPermission({ mode: 'readwrite' });
    if (permission !== 'granted') {
      const requested = await dirHandle.requestPermission({ mode: 'readwrite' });
      if (requested !== 'granted') {
        alert('Permission denied to write to directory');
        return;
      }
    }

    // Export all chunks from the store (not ChunkManager's cache)
    const allChunks = Array.from(chunks.values());
    let exportedChunks = 0;

    for (const chunk of allChunks) {
      const json = chunkManager.exportChunkDataToJSON(chunk);
      const filename = `chunk_${chunk.coord.cx}_${chunk.coord.cy}.json`;
      const fileHandle = await dirHandle.getFileHandle(filename, { create: true });
      const writable = await fileHandle.createWritable();
      await writable.write(json);
      await writable.close();
      exportedChunks++;
    }

    // Export all interiors to interiors/ subdirectory
    const allInteriors = interiorStorage.getAllInteriors();
    let exportedInteriors = 0;

    if (allInteriors.length > 0) {
      // Create or get interiors subdirectory
      const interiorsDir = await dirHandle.getDirectoryHandle('interiors', { create: true });

      for (const interior of allInteriors) {
        const json = interiorStorage.exportInteriorToJSON(interior);
        const filename = `${interior.id}.json`;
        const fileHandle = await interiorsDir.getFileHandle(filename, { create: true });
        const writable = await fileHandle.createWritable();
        await writable.write(json);
        await writable.close();
        exportedInteriors++;
      }
    }

    markAllClean();
    alert(`Exported ${exportedChunks} chunk(s) and ${exportedInteriors} interior(s) to ${dirHandle.name}/`);
  };

  // Fallback: export as downloadable ZIP file
  const exportAsZip = async () => {
    const allChunks = Array.from(chunks.values());
    const allInteriors = interiorStorage.getAllInteriors();

    if (allChunks.length === 0 && allInteriors.length === 0) {
      alert('No chunks or interiors to export');
      return;
    }

    const zip = new JSZip();
    let exportedChunks = 0;
    let exportedInteriors = 0;

    for (const chunk of allChunks) {
      const json = chunkManager.exportChunkDataToJSON(chunk);
      const filename = `chunk_${chunk.coord.cx}_${chunk.coord.cy}.json`;
      zip.file(filename, json);
      exportedChunks++;
    }

    // Add interiors to interiors/ folder in ZIP
    if (allInteriors.length > 0) {
      const interiorsFolder = zip.folder('interiors');
      for (const interior of allInteriors) {
        const json = interiorStorage.exportInteriorToJSON(interior);
        interiorsFolder?.file(`${interior.id}.json`, json);
        exportedInteriors++;
      }
    }

    // Generate and download ZIP
    const blob = await zip.generateAsync({ type: 'blob' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'map_export.zip';
    a.click();
    URL.revokeObjectURL(url);

    markAllClean();
    alert(`Downloaded map_export.zip with ${exportedChunks} chunk(s) and ${exportedInteriors} interior(s).\n\nExtract to: rust-server/maps/world_0/`);
  };

  const handleZoomIn = () => {
    setViewport({ zoom: Math.min(4, viewport.zoom * 1.25) });
  };

  const handleZoomOut = () => {
    setViewport({ zoom: Math.max(0.25, viewport.zoom / 1.25) });
  };

  const handleResetView = () => {
    setViewport({ offsetX: 400, offsetY: 200, zoom: 1 });
  };

  const handleClearLocalData = async () => {
    const confirmed = window.confirm(
      'This will clear all locally saved map data.\n\nYour current session will not be affected, but changes will be lost on refresh.\n\nAre you sure?'
    );
    if (!confirmed) return;

    try {
      await storage.clearLocal();
      alert('Local data cleared.');
    } catch (err) {
      console.error('Clear failed:', err);
      alert(`Clear failed: ${(err as Error).message}`);
    }
  };

  const handleExportMap = async () => {
    try {
      const jsonData = await storage.exportMapData();
      const blob = new Blob([jsonData], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `map-export-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Export failed:', err);
      alert(`Export failed: ${(err as Error).message}`);
    }
  };

  const handleExportMapWithInteriors = async () => {
    try {
      const jsonData = await storage.exportMapDataWithInteriors();
      const blob = new Blob([jsonData], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `map-full-export-${new Date().toISOString().split('T')[0]}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Export failed:', err);
      alert(`Export failed: ${(err as Error).message}`);
    }
  };

  const handleImportMap = () => {
    importInputRef.current?.click();
  };

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const confirmed = window.confirm(
      'This will replace all map data with the imported file.\n\nAre you sure?'
    );
    if (!confirmed) {
      e.target.value = '';
      return;
    }

    try {
      const text = await file.text();
      const count = await storage.importMapData(text);

      // Reload chunks into editor
      const loadedChunks = await storage.loadAllChunks();
      setChunks(loadedChunks, true);

      // Recalculate bounds
      let minCx = Infinity, maxCx = -Infinity;
      let minCy = Infinity, maxCy = -Infinity;
      for (const chunk of loadedChunks.values()) {
        minCx = Math.min(minCx, chunk.coord.cx);
        maxCx = Math.max(maxCx, chunk.coord.cx);
        minCy = Math.min(minCy, chunk.coord.cy);
        maxCy = Math.max(maxCy, chunk.coord.cy);
      }
      setWorldBounds({
        minCx: minCx === Infinity ? 0 : minCx,
        maxCx: maxCx === -Infinity ? 0 : maxCx,
        minCy: minCy === Infinity ? 0 : minCy,
        maxCy: maxCy === -Infinity ? 0 : maxCy,
      });

      alert(`Imported ${count} chunks successfully.`);
    } catch (err) {
      console.error('Import failed:', err);
      alert(`Import failed: ${(err as Error).message}`);
    }

    e.target.value = '';
  };

  const handleImportFullMap = () => {
    importFullInputRef.current?.click();
  };

  const handleImportFullFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const confirmed = window.confirm(
      'This will replace all map data AND interiors with the imported file.\n\nAre you sure?'
    );
    if (!confirmed) {
      e.target.value = '';
      return;
    }

    try {
      const text = await file.text();
      const result = await storage.importMapDataWithInteriors(text);

      // Reload chunks into editor
      const loadedChunks = await storage.loadAllChunks();
      setChunks(loadedChunks, true);

      // Recalculate bounds
      let minCx = Infinity, maxCx = -Infinity;
      let minCy = Infinity, maxCy = -Infinity;
      for (const chunk of loadedChunks.values()) {
        minCx = Math.min(minCx, chunk.coord.cx);
        maxCx = Math.max(maxCx, chunk.coord.cx);
        minCy = Math.min(minCy, chunk.coord.cy);
        maxCy = Math.max(maxCy, chunk.coord.cy);
      }
      setWorldBounds({
        minCx: minCx === Infinity ? 0 : minCx,
        maxCx: maxCx === -Infinity ? 0 : maxCx,
        minCy: minCy === Infinity ? 0 : minCy,
        maxCy: maxCy === -Infinity ? 0 : maxCy,
      });

      alert(`Imported ${result.chunks} chunks and ${result.interiors} interiors successfully.`);
    } catch (err) {
      console.error('Import failed:', err);
      alert(`Import failed: ${(err as Error).message}`);
    }

    e.target.value = '';
  };

  const handleSyncToServer = async () => {
    if (!isConnected) {
      alert('Not connected to server. Changes are saved locally.');
      return;
    }

    // Cancel any pending debounced save to prevent it from overwriting after we sync
    cancelPendingSave();

    try {
      const savedKeys = await storage.saveDirtyChunks(chunks);
      if (savedKeys.length > 0) {
        markAllClean();
        alert(`Synced ${savedKeys.length} changed chunk(s) to server.`);
      } else {
        alert('No changes to sync.');
      }
    } catch (err) {
      console.error('Sync failed:', err);
      alert(`Sync failed: ${(err as Error).message}`);
    }
  };

  // Interior handlers
  const handleNewInterior = () => {
    setNewInteriorId('');
    setNewInteriorName('');
    setNewInteriorWidth(16);
    setNewInteriorHeight(16);
    setShowNewInteriorModal(true);
  };

  const handleCreateInterior = () => {
    if (!newInteriorId.trim()) {
      alert('Please enter an ID for the interior map');
      return;
    }
    if (!newInteriorName.trim()) {
      alert('Please enter a name for the interior map');
      return;
    }
    if (availableInteriors.includes(newInteriorId)) {
      alert(`An interior with ID "${newInteriorId}" already exists`);
      return;
    }

    createInterior(newInteriorId.trim(), newInteriorName.trim(), newInteriorWidth, newInteriorHeight);
    setShowNewInteriorModal(false);
  };

  const handleOpenInterior = async () => {
    // Load list of available interiors
    const ids = await interiorStorage.loadInteriorList();
    setAvailableInteriors(ids);
    setShowOpenInteriorModal(true);
  };

  const handleSelectInterior = async (id: string) => {
    await loadInterior(id);
    setShowOpenInteriorModal(false);
  };

  const handleSaveInterior = async () => {
    if (currentInterior) {
      const success = await saveCurrentInterior();
      if (success) {
        alert(`Saved interior "${currentInterior.id}"`);
      } else {
        alert('Failed to save interior');
      }
    }
  };

  const handleBackToOverworld = () => {
    if (currentInterior?.dirty) {
      const confirmed = window.confirm(
        'You have unsaved changes to this interior. Discard changes?'
      );
      if (!confirmed) return;
    }
    switchToOverworld();
  };

  const handleDownloadChunk = () => {
    const parts = downloadChunkCoord.split(',').map((s) => parseInt(s.trim()));
    if (parts.length !== 2 || isNaN(parts[0]) || isNaN(parts[1])) {
      alert('Invalid coordinates. Expected format: cx, cy');
      return;
    }
    const [cx, cy] = parts;
    const chunk = chunks.get(`${cx},${cy}`);
    if (!chunk) {
      alert(`Chunk (${cx}, ${cy}) not found`);
      return;
    }
    const json = chunkManager.exportChunkDataToJSON(chunk, true);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `chunk_${cx}_${cy}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setShowDownloadChunkModal(false);
  };

  const handleDownloadInterior = () => {
    const id = downloadInteriorId.trim();
    if (!id) {
      alert('Please enter an interior map ID');
      return;
    }
    const interior = interiorStorage.getInterior(id);
    if (!interior) {
      alert(`Interior "${id}" not found. Make sure it's loaded in the editor.`);
      return;
    }
    const json = interiorStorage.exportInteriorToJSON(interior);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${id}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setShowDownloadChunkModal(false);
  };

  const handleImportChunk = () => {
    importChunkInputRef.current?.click();
  };

  const handleImportChunkFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    try {
      const text = await file.text();
      const data = JSON.parse(text);

      // Try to extract coord from the data or filename
      let coord: { cx: number; cy: number } | null = null;

      // Check if data has coord embedded
      if (data.coord && typeof data.coord.cx === 'number' && typeof data.coord.cy === 'number') {
        coord = { cx: data.coord.cx, cy: data.coord.cy };
      }

      // Try to extract from filename (e.g., "chunk_1_-2.json" or "1_-2.json")
      if (!coord) {
        const match = file.name.match(/(?:chunk_)?(-?\d+)_(-?\d+)\.json$/);
        if (match) {
          coord = { cx: parseInt(match[1]), cy: parseInt(match[2]) };
        }
      }

      if (!coord) {
        const input = window.prompt(
          'Could not detect chunk coordinates from file.\n\nEnter coordinates as "cx,cy" (e.g., "0,0" or "-1,2"):'
        );
        if (!input) {
          e.target.value = '';
          return;
        }
        const parts = input.split(',').map((s) => parseInt(s.trim()));
        if (parts.length !== 2 || isNaN(parts[0]) || isNaN(parts[1])) {
          alert('Invalid coordinates. Expected format: cx,cy');
          e.target.value = '';
          return;
        }
        coord = { cx: parts[0], cy: parts[1] };
      }

      const existingChunk = chunks.get(`${coord.cx},${coord.cy}`);
      const action = existingChunk ? 'replace' : 'add';

      const confirmed = window.confirm(
        `This will ${action} chunk (${coord.cx}, ${coord.cy}).\n\nContinue?`
      );
      if (!confirmed) {
        e.target.value = '';
        return;
      }

      // Parse chunk data
      const chunk = chunkManager.parseChunkFromData(data, coord);
      chunk.dirty = true;

      // Update the store
      const newChunks = new Map(chunks);
      newChunks.set(`${coord.cx},${coord.cy}`, chunk);
      setChunks(newChunks, false);

      // Recalculate bounds
      let minCx = Infinity, maxCx = -Infinity;
      let minCy = Infinity, maxCy = -Infinity;
      for (const c of newChunks.values()) {
        minCx = Math.min(minCx, c.coord.cx);
        maxCx = Math.max(maxCx, c.coord.cx);
        minCy = Math.min(minCy, c.coord.cy);
        maxCy = Math.max(maxCy, c.coord.cy);
      }
      setWorldBounds({
        minCx: minCx === Infinity ? 0 : minCx,
        maxCx: maxCx === -Infinity ? 0 : maxCx,
        minCy: minCy === Infinity ? 0 : minCy,
        maxCy: maxCy === -Infinity ? 0 : maxCy,
      });

      alert(`Imported chunk (${coord.cx}, ${coord.cy}) successfully.`);
    } catch (err) {
      console.error('Chunk import failed:', err);
      alert(`Import failed: ${(err as Error).message}`);
    }

    e.target.value = '';
  };

  const handleResizeInterior = () => {
    if (currentInterior) {
      setResizeWidth(currentInterior.width);
      setResizeHeight(currentInterior.height);
      setShowResizeInteriorModal(true);
    }
  };

  const handleConfirmResize = () => {
    if (!currentInterior) return;
    if (resizeWidth === currentInterior.width && resizeHeight === currentInterior.height) {
      setShowResizeInteriorModal(false);
      return;
    }
    const confirmed = window.confirm(
      `Resize "${currentInterior.name}" from ${currentInterior.width}x${currentInterior.height} to ${resizeWidth}x${resizeHeight}?\n\nTiles outside the new bounds will be removed. This cannot be undone.`
    );
    if (!confirmed) return;
    resizeInterior(resizeWidth, resizeHeight);
    setShowResizeInteriorModal(false);
  };

  return (
    <div className={styles.menuBar}>
      <input
        ref={importInputRef}
        type="file"
        accept=".json"
        style={{ display: 'none' }}
        onChange={handleImportFile}
      />
      <input
        ref={importFullInputRef}
        type="file"
        accept=".json"
        style={{ display: 'none' }}
        onChange={handleImportFullFile}
      />
      <input
        ref={importChunkInputRef}
        type="file"
        accept=".json"
        style={{ display: 'none' }}
        onChange={handleImportChunkFile}
      />
      <div className={styles.menu}>
        <div className={styles.menuItem}>
          <span className={styles.menuTitle}>File</span>
          <div className={styles.dropdown}>
            {editorMode === 'overworld' ? (
              <>
                <div className={styles.dropdownLabel}>Save</div>
                <button className={styles.dropdownItem} onClick={handleSyncToServer}>
                  Sync to Server
                </button>
                <button className={styles.dropdownItem} onClick={handleSaveAll}>
                  Download Modified ({getDirtyChunks().length})
                </button>
                <div className={styles.separator} />
                <div className={styles.dropdownLabel}>Export</div>
                <button className={styles.dropdownItem} onClick={handleExportToServer}>
                  Export to Directory...
                </button>
                <button className={styles.dropdownItem} onClick={handleExportMap}>
                  Export Map (JSON)
                </button>
                <button className={styles.dropdownItem} onClick={handleExportMapWithInteriors}>
                  Export Map + Interiors (JSON)
                </button>
                <button className={styles.dropdownItem} onClick={() => setShowDownloadChunkModal(true)}>
                  Download Map Data (JSON)
                </button>
                <div className={styles.separator} />
                <div className={styles.dropdownLabel}>Import</div>
                <button className={styles.dropdownItem} onClick={handleImportMap}>
                  Import Map (JSON)
                </button>
                <button className={styles.dropdownItem} onClick={handleImportFullMap}>
                  Import Map + Interiors (JSON)
                </button>
                <button className={styles.dropdownItem} onClick={handleImportChunk}>
                  Import Chunk (JSON)
                </button>
                <div className={styles.separator} />
                <div className={styles.dropdownLabel}>Danger Zone</div>
                <button className={styles.dropdownItem} onClick={handleClearLocalData}>
                  Clear Local Storage
                </button>
              </>
            ) : (
              <>
                <button className={styles.dropdownItem} onClick={handleSaveInterior}>
                  Save Interior {currentInterior?.dirty ? '*' : ''}
                </button>
                <button className={styles.dropdownItem} onClick={handleResizeInterior}>
                  Resize Interior...
                </button>
                <div className={styles.separator} />
                <button className={styles.dropdownItem} onClick={handleBackToOverworld}>
                  Back to Overworld
                </button>
              </>
            )}
            <div className={styles.separator} />
            <div className={styles.dropdownLabel}>Interiors</div>
            <button className={styles.dropdownItem} onClick={handleNewInterior}>
              New Interior Map...
            </button>
            <button className={styles.dropdownItem} onClick={handleOpenInterior}>
              Open Interior Map...
            </button>
          </div>
        </div>

        <div className={styles.menuItem}>
          <span className={styles.menuTitle}>Edit</span>
          <div className={styles.dropdown}>
            <button
              className={styles.dropdownItem}
              onClick={() => history.undo()}
              disabled={!history.canUndo()}
            >
              Undo {history.getUndoDescription() ? `(${history.getUndoDescription()})` : ''}
            </button>
            <button
              className={styles.dropdownItem}
              onClick={() => history.redo()}
              disabled={!history.canRedo()}
            >
              Redo {history.getRedoDescription() ? `(${history.getRedoDescription()})` : ''}
            </button>
          </div>
        </div>

        <div className={styles.menuItem}>
          <span className={styles.menuTitle}>View</span>
          <div className={styles.dropdown}>
            <button className={styles.dropdownItem} onClick={toggleGrid}>
              {showGrid ? '✓ ' : '  '}Show Grid
            </button>
            <button className={styles.dropdownItem} onClick={toggleChunkBounds}>
              {showChunkBounds ? '✓ ' : '  '}Show Chunk Bounds
            </button>
            <div className={styles.separator} />
            <button className={styles.dropdownItem} onClick={togglePaletteSide}>
              Palettes on {paletteSide === 'left' ? 'Right' : 'Left'}
            </button>
            <div className={styles.separator} />
            <button className={styles.dropdownItem} onClick={handleZoomIn}>
              Zoom In
            </button>
            <button className={styles.dropdownItem} onClick={handleZoomOut}>
              Zoom Out
            </button>
            <button className={styles.dropdownItem} onClick={handleResetView}>
              Reset View
            </button>
          </div>
        </div>

        <div className={styles.menuItem}>
          <span className={styles.menuTitle}>Assets</span>
          <div className={styles.dropdown}>
            <button className={styles.dropdownItem} onClick={() => openAssetManager('objects')}>
              Import Objects...
            </button>
            <button className={styles.dropdownItem} onClick={() => openAssetManager('walls')}>
              Import Walls...
            </button>
            <button className={styles.dropdownItem} onClick={() => openAssetManager('tiles')}>
              Import Tiles...
            </button>
            <div className={styles.separator} />
            <button
              className={styles.dropdownItem}
              disabled={rebuildingAtlas}
              onClick={async () => {
                setRebuildingAtlas(true);
                try {
                  const resp = await fetch('/mapper/api/assets/rebuild-atlas', { method: 'POST' });
                  const result = await resp.json();
                  if (result.success) {
                    alert(`Atlas rebuilt in ${result.duration}ms`);
                  } else {
                    alert(`Atlas rebuild failed: ${result.error}`);
                  }
                } catch (err) {
                  alert(`Rebuild failed: ${(err as Error).message}`);
                } finally {
                  setRebuildingAtlas(false);
                }
              }}
            >
              {rebuildingAtlas ? 'Rebuilding...' : 'Rebuild Atlases'}
            </button>
          </div>
        </div>
        <button className={styles.studioButton} onClick={onOpenContentStudio}>
          Content Studio
        </button>
      </div>

      <div className={styles.status}>
        {availableWorlds.length > 1 ? (
          <select
            className={styles.worldSelector}
            value={currentWorld}
            onChange={(e) => switchWorld(e.target.value)}
          >
            {availableWorlds.map((w) => (
              <option key={w} value={w}>{w}</option>
            ))}
          </select>
        ) : (
          <span className={styles.statusItem}>{currentWorld}</span>
        )}
        {editorMode === 'interior' && currentInteriorId && (
          <span className={styles.statusItem}>
            Editing: {currentInteriorId} {currentInterior?.dirty ? '*' : ''}
          </span>
        )}
        <span className={styles.statusItem}>Zoom: {Math.round(viewport.zoom * 100)}%</span>
        {editorMode === 'overworld' && (
          <span className={styles.statusItem}>
            {getDirtyChunks().length > 0 && `${getDirtyChunks().length} unsaved`}
          </span>
        )}
        <span className={`${styles.statusItem} ${styles.connectionStatus} ${isConnected ? styles.connected : styles.disconnected}`}>
          {isConnected ? 'Connected' : 'Offline'}
        </span>
      </div>

      {/* Modals portaled to document.body to avoid backdrop-filter containing block */}
      {showNewInteriorModal && createPortal(
        <div className={styles.modalOverlay} onClick={() => setShowNewInteriorModal(false)}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <h3>New Interior Map</h3>
            <div className={styles.modalField}>
              <label>ID (no spaces)</label>
              <input
                type="text"
                value={newInteriorId}
                onChange={(e) => setNewInteriorId(e.target.value.replace(/\s/g, '_'))}
                placeholder="e.g., blacksmith_shop"
                autoFocus
              />
            </div>
            <div className={styles.modalField}>
              <label>Display Name</label>
              <input
                type="text"
                value={newInteriorName}
                onChange={(e) => setNewInteriorName(e.target.value)}
                placeholder="e.g., Blacksmith's Workshop"
              />
            </div>
            <div className={styles.modalFieldRow}>
              <div className={styles.modalField}>
                <label>Width (tiles)</label>
                <input
                  type="number"
                  value={newInteriorWidth}
                  onChange={(e) => setNewInteriorWidth(Math.max(4, parseInt(e.target.value) || 16))}
                  min={4}
                  max={64}
                />
              </div>
              <div className={styles.modalField}>
                <label>Height (tiles)</label>
                <input
                  type="number"
                  value={newInteriorHeight}
                  onChange={(e) => setNewInteriorHeight(Math.max(4, parseInt(e.target.value) || 16))}
                  min={4}
                  max={64}
                />
              </div>
            </div>
            <div className={styles.modalActions}>
              <button onClick={() => setShowNewInteriorModal(false)}>Cancel</button>
              <button onClick={handleCreateInterior} className={styles.primaryButton}>Create</button>
            </div>
          </div>
        </div>,
        document.body
      )}

      {showOpenInteriorModal && createPortal(
        <div className={styles.modalOverlay} onClick={() => setShowOpenInteriorModal(false)}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <h3>Open Interior Map</h3>
            {availableInteriors.length === 0 ? (
              <p className={styles.emptyMessage}>No interior maps found</p>
            ) : (
              <div className={styles.interiorList}>
                {availableInteriors.map((id) => (
                  <button
                    key={id}
                    className={styles.interiorItem}
                    onClick={() => handleSelectInterior(id)}
                  >
                    {id}
                  </button>
                ))}
              </div>
            )}
            <div className={styles.modalActions}>
              <button onClick={() => setShowOpenInteriorModal(false)}>Cancel</button>
            </div>
          </div>
        </div>,
        document.body
      )}

      {showDownloadChunkModal && createPortal(
        <div className={styles.modalOverlay} onClick={() => setShowDownloadChunkModal(false)}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <h3>Download Map Data</h3>
            <div className={styles.modalField}>
              <label>Chunk Coordinate (cx, cy)</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="text"
                  value={downloadChunkCoord}
                  onChange={(e) => setDownloadChunkCoord(e.target.value)}
                  placeholder="0, 0"
                  autoFocus
                  onKeyDown={(e) => { if (e.key === 'Enter') handleDownloadChunk(); }}
                  style={{ flex: 1 }}
                />
                <button className={styles.primaryButton} onClick={handleDownloadChunk}>
                  Download
                </button>
              </div>
            </div>
            <div className={styles.separator} />
            <div className={styles.modalField}>
              <label>Interior Map ID</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="text"
                  value={downloadInteriorId}
                  onChange={(e) => setDownloadInteriorId(e.target.value)}
                  placeholder="e.g., blacksmith_shop"
                  onKeyDown={(e) => { if (e.key === 'Enter') handleDownloadInterior(); }}
                  style={{ flex: 1 }}
                />
                <button className={styles.primaryButton} onClick={handleDownloadInterior}>
                  Download
                </button>
              </div>
            </div>
            <div className={styles.modalActions}>
              <button onClick={() => setShowDownloadChunkModal(false)}>Close</button>
            </div>
          </div>
        </div>,
        document.body
      )}

      {showResizeInteriorModal && currentInterior && createPortal(
        <div className={styles.modalOverlay} onClick={() => setShowResizeInteriorModal(false)}>
          <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
            <h3>Resize Interior Map</h3>
            <p style={{ fontSize: '0.85em', color: '#aaa', margin: '0 0 12px' }}>
              Current size: {currentInterior.width} x {currentInterior.height}
            </p>
            <div className={styles.modalFieldRow}>
              <div className={styles.modalField}>
                <label>Width (tiles)</label>
                <input
                  type="number"
                  value={resizeWidth}
                  onChange={(e) => setResizeWidth(Math.max(4, parseInt(e.target.value) || 4))}
                  min={4}
                  max={64}
                  autoFocus
                />
              </div>
              <div className={styles.modalField}>
                <label>Height (tiles)</label>
                <input
                  type="number"
                  value={resizeHeight}
                  onChange={(e) => setResizeHeight(Math.max(4, parseInt(e.target.value) || 4))}
                  min={4}
                  max={64}
                />
              </div>
            </div>
            <div className={styles.modalActions}>
              <button onClick={() => setShowResizeInteriorModal(false)}>Cancel</button>
              <button onClick={handleConfirmResize} className={styles.primaryButton}>Resize</button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}
