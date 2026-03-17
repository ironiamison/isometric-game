import { useEffect, useRef, useCallback, useState } from 'react';
import { useEditorStore } from '@/state/store';
import { isometricRenderer } from '@/core/IsometricRenderer';
import { screenToWorldTile, worldToChunk, getBrushTiles } from '@/core/coords';
import { Tool, Layer } from '@/types';
import type { DevNote } from '@/types';
import { history } from '@/core/History';
import { ContextMenu } from '@/components/ContextMenu';
import { notesStorage } from '@/core/NotesStorage';
import { objectLoader } from '@/core/ObjectLoader';
import styles from './Canvas.module.css';

export function Canvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isPanning, setIsPanning] = useState(false);
  const [isPainting, setIsPainting] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; wx: number; wy: number } | null>(null);
  const lastMousePos = useRef({ x: 0, y: 0 });
  const heightDelta = useRef(0); // +1 for raise, -1 for lower (during drag)

  const {
    chunks,
    viewport,
    hoveredTile,
    activeTool,
    activeLayer,
    brushSize,
    selectedTileId,
    selectedEntityId,
    selectedObjectId,
    selectedTiles,
    selectedPortal,
    showGrid,
    showChunkBounds,
    showCollision,
    showEntities,
    showMapObjects,
    showPortals,
    showNotes,
    visibleLayers,
    notes,
    selectedNoteId,
    editorMode,
    currentInterior,
    pan,
    zoom,
    setHoveredTile,
    setTile,
    toggleCollision,
    fillTiles,
    magicWandSelect,
    fillSelectedTiles,
    addEntity,
    removeEntity,
    addMapObject,
    removeMapObject,
    addPortal,
    adjustHeight,
    setBlockType,
    selectedBlockTypeDown,
    selectedBlockTypeRight,
    toggleGatheringZone,
    findGatheringZoneAtWorld,
    setSelectedGatheringZone,
    setBrushSize,
    setSelectedTileId,
    findEntityAtWorld,
    setSelectedEntitySpawn,
    findMapObjectAtWorld,
    setSelectedMapObject,
    findPortalAtWorld,
    setSelectedPortal,
    toggleWall,
    setInteriorTile,
    toggleInteriorCollision,
    fillInteriorTiles,
    addInteriorEntity,
    removeInteriorEntity,
    addInteriorMapObject,
    removeInteriorMapObject,
    toggleInteriorWall,
    addSpawnPoint,
    removeSpawnPoint,
    addExitPortal,
    removeExitPortal,
    findExitPortalAt,
    setSelectedExitPortal,
    setSelectedInteriorEntity,
    setSelectedInteriorMapObject,
    setSelectedInteriorWall,
    toggleInteriorGatheringZone,
    findWallAtWorld,
    setSelectedWall,
    addNote,
    setSelectedNoteId,
    setShowNotes,
    setNotesPanelCollapsed,
  } = useEditorStore();

  // Setup canvas and renderer
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    isometricRenderer.attach(canvas);

    const resizeObserver = new ResizeObserver(() => {
      if (containerRef.current) {
        canvas.width = containerRef.current.clientWidth;
        canvas.height = containerRef.current.clientHeight;
      }
    });

    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
      canvas.width = containerRef.current.clientWidth;
      canvas.height = containerRef.current.clientHeight;
    }

    return () => {
      resizeObserver.disconnect();
      isometricRenderer.detach();
    };
  }, []);

  // Arrow key camera movement
  const heldKeys = useRef<Set<string>>(new Set());

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
        heldKeys.current.add(e.key);
        e.preventDefault();
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      heldKeys.current.delete(e.key);
    };

    let animationId: number;
    const panLoop = () => {
      const panAmount = 5;
      let dx = 0;
      let dy = 0;

      if (heldKeys.current.has('ArrowUp')) dy += panAmount;
      if (heldKeys.current.has('ArrowDown')) dy -= panAmount;
      if (heldKeys.current.has('ArrowLeft')) dx += panAmount;
      if (heldKeys.current.has('ArrowRight')) dx -= panAmount;

      if (dx !== 0 || dy !== 0) {
        pan(dx, dy);
      }

      animationId = requestAnimationFrame(panLoop);
    };

    animationId = requestAnimationFrame(panLoop);
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      cancelAnimationFrame(animationId);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    };
  }, [pan]);

  // Update renderer options
  useEffect(() => {
    isometricRenderer.setOptions({
      showGrid,
      showChunkBounds,
      showCollision,
      showEntities,
      showMapObjects,
      showPortals,
      showNotes,
      showHeights: activeTool === Tool.HeightRaise,
      visibleLayers,
    });
  }, [showGrid, showChunkBounds, showCollision, showEntities, showMapObjects, showPortals, showNotes, visibleLayers, activeTool]);

  // Pass notes to renderer
  useEffect(() => {
    isometricRenderer.setNotes(notes, selectedNoteId);
  }, [notes, selectedNoteId]);

  // Render loop
  useEffect(() => {
    let animationId: number;

    const render = () => {
      if (editorMode === 'interior' && currentInterior) {
        // Render interior map
        isometricRenderer.renderInterior(currentInterior, viewport);
      } else {
        // Render overworld chunks
        const allChunks = Array.from(chunks.values());
        isometricRenderer.render(allChunks, viewport);

        // Highlight selected tiles (magic wand selection)
        if (selectedTiles.size > 0) {
          for (const tileKey of selectedTiles) {
            const [wxStr, wyStr] = tileKey.split(',');
            const wx = parseInt(wxStr, 10);
            const wy = parseInt(wyStr, 10);
            isometricRenderer.highlightTile({ wx, wy }, viewport, 'rgba(0, 150, 255, 0.4)', true);
          }
        }

        // Highlight selected portal
        if (selectedPortal) {
          const chunk = chunks.get(`${selectedPortal.chunkCoord.cx},${selectedPortal.chunkCoord.cy}`);
          if (chunk && chunk.portals) {
            const portal = chunk.portals.find((p) => p.id === selectedPortal.portalId);
            if (portal) {
              const baseX = selectedPortal.chunkCoord.cx * 32;
              const baseY = selectedPortal.chunkCoord.cy * 32;
              for (let py = 0; py < portal.height; py++) {
                for (let px = 0; px < portal.width; px++) {
                  isometricRenderer.highlightTile(
                    { wx: baseX + portal.x + px, wy: baseY + portal.y + py },
                    viewport,
                    'rgba(200, 0, 255, 0.6)',
                    true
                  );
                }
              }
            }
          }
        }
      }

      // Highlight brush area (works for both modes)
      if (hoveredTile) {
        const brushTiles = getBrushTiles(hoveredTile, brushSize);
        for (const t of brushTiles) {
          isometricRenderer.highlightTile(t, viewport, 'rgba(255, 255, 255, 0.4)', true);
        }
        // Outline the center tile more prominently
        isometricRenderer.highlightTile(hoveredTile, viewport, '#ffffff');
      }

      animationId = requestAnimationFrame(render);
    };

    render();

    return () => {
      cancelAnimationFrame(animationId);
    };
  }, [chunks, viewport, hoveredTile, brushSize, selectedTiles, selectedPortal, editorMode, currentInterior]);

  // Handle tool action at position
  const handleToolAction = useCallback(
    (clientX: number, clientY: number) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const screenX = clientX - rect.left;
      const screenY = clientY - rect.top;
      const worldTile = screenToWorldTile({ sx: screenX, sy: screenY }, viewport);

      const tiles = getBrushTiles(worldTile, brushSize);

      // Interior mode handling
      if (editorMode === 'interior' && currentInterior) {
        switch (activeTool) {
          case Tool.Paint:
            if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
              for (const t of tiles) {
                if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                  setInteriorTile(t.wx, t.wy, activeLayer, selectedTileId);
                }
              }
            }
            break;
          case Tool.Eraser:
            if (activeLayer === Layer.Entities) {
              for (const t of tiles) {
                const entity = currentInterior.entities.find(
                  (e) => e.x === t.wx && e.y === t.wy
                );
                if (entity) removeInteriorEntity(entity.id);
              }
            } else if (activeLayer === Layer.MapObjects) {
              for (const t of tiles) {
                const obj = currentInterior.mapObjects.find(
                  (o) => o.x === t.wx && o.y === t.wy
                );
                if (obj) removeInteriorMapObject(obj.id);
              }
            } else if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
              for (const t of tiles) {
                if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                  setInteriorTile(t.wx, t.wy, activeLayer, 0);
                }
              }
            }
            break;
          case Tool.Collision:
            for (const t of tiles) {
              if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                toggleInteriorCollision(t.wx, t.wy);
              }
            }
            break;
          case Tool.Fill:
            if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
              fillInteriorTiles(worldTile.wx, worldTile.wy, activeLayer, selectedTileId);
            }
            break;
          case Tool.Entity: {
            const existingInteriorEntity = currentInterior.entities.find(
              (e) => e.x === worldTile.wx && e.y === worldTile.wy
            );
            if (existingInteriorEntity) {
              setSelectedInteriorEntity(existingInteriorEntity.id);
              setSelectedInteriorMapObject(null);
            } else if (selectedEntityId) {
              for (const t of tiles) {
                if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                  addInteriorEntity(t.wx, t.wy, selectedEntityId);
                }
              }
            }
            break;
          }
          case Tool.Object: {
            const existingInteriorObj = currentInterior.mapObjects.find(
              (o) => o.x === worldTile.wx && o.y === worldTile.wy
            );
            if (existingInteriorObj) {
              setSelectedInteriorMapObject(existingInteriorObj.id);
              setSelectedInteriorEntity(null);
            } else if (selectedObjectId) {
              const objDef = objectLoader.getObject(selectedObjectId);
              if (objDef) {
                for (const t of tiles) {
                  if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                    addInteriorMapObject(t.wx, t.wy, selectedObjectId, objDef.width, objDef.height);
                  }
                }
              }
            }
            break;
          }
          case Tool.Select: {
            const entityAtInteriorPos = currentInterior.entities.find(
              (e) => e.x === worldTile.wx && e.y === worldTile.wy
            );
            if (entityAtInteriorPos) {
              setSelectedInteriorEntity(entityAtInteriorPos.id);
              setSelectedInteriorMapObject(null);
              setSelectedInteriorWall(null);
            } else {
              const objAtInteriorPos = currentInterior.mapObjects.find(
                (o) => o.x === worldTile.wx && o.y === worldTile.wy
              );
              if (objAtInteriorPos) {
                setSelectedInteriorMapObject(objAtInteriorPos.id);
                setSelectedInteriorEntity(null);
                setSelectedInteriorWall(null);
              } else {
                const wallAtInteriorPos = currentInterior.walls.find(
                  (w) => w.x === worldTile.wx && w.y === worldTile.wy
                );
                if (wallAtInteriorPos) {
                  setSelectedInteriorWall(wallAtInteriorPos.id);
                  setSelectedInteriorEntity(null);
                  setSelectedInteriorMapObject(null);
                } else {
                  setSelectedInteriorEntity(null);
                  setSelectedInteriorMapObject(null);
                  setSelectedInteriorWall(null);
                }
              }
            }
            break;
          }
          case Tool.Eyedropper: {
            const index = worldTile.wy * currentInterior.width + worldTile.wx;
            const layerKey =
              activeLayer === Layer.Ground
                ? 'ground'
                : activeLayer === Layer.Objects
                  ? 'objects'
                  : 'overhead';
            if (layerKey === 'ground' || layerKey === 'objects' || layerKey === 'overhead') {
              const tileId = currentInterior.layers[layerKey][index];
              if (tileId > 0) {
                setSelectedTileId(tileId);
              }
            }
            break;
          }
          case Tool.WallDown:
          case Tool.WallRight: {
            if (selectedObjectId) {
              const wallDef = objectLoader.getWall(selectedObjectId);
              const objDef = wallDef || objectLoader.getObject(selectedObjectId);
              if (objDef) {
                const edge = activeTool === Tool.WallDown ? 'down' : 'right';
                const gid = wallDef
                  ? objectLoader.wallIdToGid(selectedObjectId)
                  : objectLoader.idToGid(selectedObjectId);
                for (const t of tiles) {
                  if (t.wx >= 0 && t.wx < currentInterior.width && t.wy >= 0 && t.wy < currentInterior.height) {
                    toggleInteriorWall(t.wx, t.wy, edge, gid);
                  }
                }
              }
            }
            break;
          }
          case Tool.SpawnPoint: {
            const existingSpawn = currentInterior.spawnPoints.find(
              (sp) => sp.x === worldTile.wx && sp.y === worldTile.wy
            );
            if (existingSpawn) {
              removeSpawnPoint(existingSpawn.name);
            } else {
              const name = prompt('Enter spawn point name:', `spawn_${currentInterior.spawnPoints.length}`);
              if (name) {
                addSpawnPoint(name, worldTile.wx, worldTile.wy);
              }
            }
            break;
          }
          case Tool.ExitPortal: {
            const existingExit = findExitPortalAt(worldTile.wx, worldTile.wy);
            if (existingExit) {
              setSelectedExitPortal({ portalId: existingExit.id });
            } else {
              const newPortal = addExitPortal(worldTile.wx, worldTile.wy);
              setSelectedExitPortal({ portalId: newPortal.id });
            }
            break;
          }
          case Tool.GatheringZone: {
            if (worldTile.wx >= 0 && worldTile.wx < currentInterior.width && worldTile.wy >= 0 && worldTile.wy < currentInterior.height) {
              toggleInteriorGatheringZone(worldTile.wx, worldTile.wy);
            }
            break;
          }
        }
        return;
      }

      // Overworld mode handling
      switch (activeTool) {
        case Tool.Paint:
          if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
            if (selectedTiles.size > 0) {
              fillSelectedTiles(activeLayer, selectedTileId);
            } else {
              for (const t of tiles) {
                setTile(t, activeLayer, selectedTileId);
              }
            }
          }
          break;
        case Tool.Eraser: {
          if (activeLayer === Layer.Entities) {
            for (const t of tiles) {
              const entityAtPos = findEntityAtWorld(t);
              if (entityAtPos) {
                removeEntity(entityAtPos.chunkCoord, entityAtPos.entity.id);
              }
            }
          } else if (activeLayer === Layer.MapObjects) {
            for (const t of tiles) {
              const objectAtPos = findMapObjectAtWorld(t);
              if (objectAtPos) {
                removeMapObject(objectAtPos.chunkCoord, objectAtPos.object.id);
              }
            }
          } else if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
            for (const t of tiles) {
              setTile(t, activeLayer, 0);
            }
          }
          break;
        }
        case Tool.Collision:
          for (const t of tiles) {
            toggleCollision(t);
          }
          break;
        case Tool.Fill:
          if (activeLayer === Layer.Ground || activeLayer === Layer.Objects || activeLayer === Layer.Overhead) {
            fillTiles(worldTile, activeLayer, selectedTileId);
          }
          break;
        case Tool.MagicWand:
          if (activeLayer !== Layer.Collision && activeLayer !== Layer.Entities && activeLayer !== Layer.MapObjects) {
            magicWandSelect(worldTile, activeLayer);
          }
          break;
        case Tool.Entity: {
          const existingEntity = findEntityAtWorld(worldTile);
          if (existingEntity) {
            setSelectedEntitySpawn({
              chunkCoord: existingEntity.chunkCoord,
              spawnId: existingEntity.entity.id,
            });
          } else if (selectedEntityId) {
            for (const t of tiles) {
              addEntity(t, selectedEntityId);
            }
          }
          break;
        }
        case Tool.Object: {
          const existingObject = findMapObjectAtWorld(worldTile);
          if (existingObject) {
            setSelectedMapObject({
              chunkCoord: existingObject.chunkCoord,
              objectId: existingObject.object.id,
            });
          } else if (selectedObjectId) {
            const objDef = objectLoader.getObject(selectedObjectId);
            if (objDef) {
              for (const t of tiles) {
                addMapObject(t, selectedObjectId, objDef.width, objDef.height);
              }
            }
          }
          break;
        }
        case Tool.Select: {
          const entityAtPos = findEntityAtWorld(worldTile);
          if (entityAtPos) {
            setSelectedEntitySpawn({
              chunkCoord: entityAtPos.chunkCoord,
              spawnId: entityAtPos.entity.id,
            });
            setSelectedMapObject(null);
            setSelectedWall(null);
          } else {
            const objectAtPos = findMapObjectAtWorld(worldTile);
            if (objectAtPos) {
              setSelectedMapObject({
                chunkCoord: objectAtPos.chunkCoord,
                objectId: objectAtPos.object.id,
              });
              setSelectedEntitySpawn(null);
              setSelectedWall(null);
            } else {
              const wallAtPos = findWallAtWorld(worldTile);
              if (wallAtPos) {
                setSelectedWall({
                  chunkCoord: wallAtPos.chunkCoord,
                  wallId: wallAtPos.wall.id,
                });
                setSelectedEntitySpawn(null);
                setSelectedMapObject(null);
              } else {
                setSelectedEntitySpawn(null);
                setSelectedMapObject(null);
                setSelectedWall(null);
              }
            }
          }
          break;
        }
        case Tool.Eyedropper: {
          const chunk = useEditorStore.getState().getChunk({
            cx: Math.floor(worldTile.wx / 32),
            cy: Math.floor(worldTile.wy / 32),
          });
          if (chunk) {
            const lx = ((worldTile.wx % 32) + 32) % 32;
            const ly = ((worldTile.wy % 32) + 32) % 32;
            const index = ly * 32 + lx;
            const layerKey =
              activeLayer === Layer.Ground
                ? 'ground'
                : activeLayer === Layer.Objects
                  ? 'objects'
                  : 'overhead';
            if (layerKey === 'ground' || layerKey === 'objects' || layerKey === 'overhead') {
              const tileId = chunk.layers[layerKey][index];
              if (tileId > 0) {
                setSelectedTileId(tileId);
              }
            }
          }
          break;
        }
        case Tool.WallDown:
        case Tool.WallRight: {
          if (selectedObjectId) {
            const wallDef = objectLoader.getWall(selectedObjectId);
            const objDef = wallDef || objectLoader.getObject(selectedObjectId);
            if (objDef) {
              const edge = activeTool === Tool.WallDown ? 'down' : 'right';
              const gid = wallDef
                ? objectLoader.wallIdToGid(selectedObjectId)
                : objectLoader.idToGid(selectedObjectId);
              for (const t of tiles) {
                toggleWall(t, edge, gid);
              }
            }
          }
          break;
        }
        case Tool.Portal: {
          const existingPortal = findPortalAtWorld(worldTile);
          if (existingPortal) {
            setSelectedPortal({
              chunkCoord: existingPortal.chunkCoord,
              portalId: existingPortal.portal.id,
            });
          } else {
            const newPortal = addPortal(worldTile);
            setSelectedPortal({
              chunkCoord: useEditorStore.getState().getChunk({
                cx: Math.floor(worldTile.wx / 32),
                cy: Math.floor(worldTile.wy / 32),
              })?.coord || { cx: Math.floor(worldTile.wx / 32), cy: Math.floor(worldTile.wy / 32) },
              portalId: newPortal.id,
            });
          }
          break;
        }
        case Tool.GatheringZone: {
          const existingZone = findGatheringZoneAtWorld(worldTile);
          if (existingZone) {
            setSelectedGatheringZone({
              chunkCoord: existingZone.chunkCoord,
              zoneId: existingZone.zone.id,
            });
          } else {
            toggleGatheringZone(worldTile);
            setSelectedGatheringZone(null);
          }
          break;
        }
        case Tool.HeightRaise:
          for (const t of tiles) {
            adjustHeight(t, 1);
          }
          break;
        case Tool.BlockType:
          for (const t of tiles) {
            setBlockType(t, selectedBlockTypeDown, selectedBlockTypeRight);
          }
          break;
      }
    },
    [
      viewport,
      activeTool,
      activeLayer,
      brushSize,
      selectedTileId,
      selectedEntityId,
      selectedObjectId,
      selectedTiles,
      editorMode,
      currentInterior,
      setTile,
      toggleCollision,
      fillTiles,
      fillSelectedTiles,
      addEntity,
      removeEntity,
      addMapObject,
      removeMapObject,
      addPortal,
      toggleGatheringZone,
      findGatheringZoneAtWorld,
      setSelectedGatheringZone,
      setSelectedTileId,
      findEntityAtWorld,
      setSelectedEntitySpawn,
      findMapObjectAtWorld,
      setSelectedMapObject,
      findPortalAtWorld,
      setSelectedPortal,
      adjustHeight,
      toggleWall,
      setInteriorTile,
      toggleInteriorCollision,
      fillInteriorTiles,
      addInteriorEntity,
      removeInteriorEntity,
      addInteriorMapObject,
      removeInteriorMapObject,
      toggleInteriorWall,
      toggleInteriorGatheringZone,
      addSpawnPoint,
      removeSpawnPoint,
      addExitPortal,
      removeExitPortal,
      findExitPortalAt,
      setSelectedExitPortal,
      setSelectedInteriorEntity,
      setSelectedInteriorMapObject,
      setSelectedInteriorWall,
      findWallAtWorld,
      setSelectedWall,
    ]
  );

  // Mouse event handlers
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      lastMousePos.current = { x: e.clientX, y: e.clientY };

      // Middle mouse or space+left click for panning
      if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
        setIsPanning(true);
        e.preventDefault();
        return;
      }

      // Right click for height lowering
      if (e.button === 2 && activeTool === Tool.HeightRaise) {
        e.preventDefault();
        setIsPainting(true);
        heightDelta.current = -1;
        history.beginGroup('height lower stroke');
        const canvas = canvasRef.current;
        if (canvas) {
          const rect = canvas.getBoundingClientRect();
          const screenX = e.clientX - rect.left;
          const screenY = e.clientY - rect.top;
          const worldTile = screenToWorldTile({ sx: screenX, sy: screenY }, viewport);
          const brushTiles = getBrushTiles(worldTile, brushSize);
          for (const t of brushTiles) {
            adjustHeight(t, -1);
          }
        }
        return;
      }

      // Left click for tool action
      if (e.button === 0) {
        setIsPainting(true);
        if (activeTool === Tool.Paint || activeTool === Tool.Eraser) {
          history.beginGroup(`${activeTool} stroke`);
        }
        if (activeTool === Tool.HeightRaise) {
          heightDelta.current = 1;
          history.beginGroup('height raise stroke');
        }
        if (activeTool === Tool.BlockType) {
          history.beginGroup('block type stroke');
        }
        handleToolAction(e.clientX, e.clientY);
      }
    },
    [activeTool, handleToolAction, viewport, adjustHeight]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      const worldTile = screenToWorldTile({ sx: screenX, sy: screenY }, viewport);
      setHoveredTile(worldTile);

      if (isPanning) {
        const dx = e.clientX - lastMousePos.current.x;
        const dy = e.clientY - lastMousePos.current.y;
        pan(dx, dy);
        lastMousePos.current = { x: e.clientX, y: e.clientY };
        return;
      }

      if (isPainting && (activeTool === Tool.Paint || activeTool === Tool.Eraser)) {
        handleToolAction(e.clientX, e.clientY);
      }

      if (isPainting && activeTool === Tool.HeightRaise) {
        const brushTiles = getBrushTiles(worldTile, brushSize);
        for (const t of brushTiles) {
          adjustHeight(t, heightDelta.current);
        }
      }

      if (isPainting && activeTool === Tool.BlockType) {
        const brushTiles = getBrushTiles(worldTile, brushSize);
        for (const t of brushTiles) {
          setBlockType(t, selectedBlockTypeDown, selectedBlockTypeRight);
        }
      }
    },
    [viewport, isPanning, isPainting, activeTool, brushSize, pan, setHoveredTile, handleToolAction, adjustHeight, setBlockType, selectedBlockTypeDown, selectedBlockTypeRight]
  );

  const handleMouseUp = useCallback(() => {
    if (isPainting && (activeTool === Tool.Paint || activeTool === Tool.Eraser || activeTool === Tool.HeightRaise || activeTool === Tool.BlockType)) {
      history.endGroup();
    }
    setIsPanning(false);
    setIsPainting(false);
  }, [isPainting, activeTool]);

  const handleMouseLeave = useCallback(() => {
    setHoveredTile(null);
    if (isPainting && (activeTool === Tool.Paint || activeTool === Tool.Eraser || activeTool === Tool.HeightRaise || activeTool === Tool.BlockType)) {
      history.endGroup();
    }
    setIsPanning(false);
    setIsPainting(false);
  }, [isPainting, activeTool, setHoveredTile]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        // CTRL+scroll to change brush size
        const delta = e.deltaY > 0 ? -1 : 1;
        setBrushSize(brushSize + delta);
        return;
      }
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      const rect = canvasRef.current?.getBoundingClientRect();
      if (rect) {
        zoom(factor, e.clientX - rect.left, e.clientY - rect.top);
      }
    },
    [zoom, brushSize, setBrushSize]
  );


  return (
    <div ref={containerRef} className={styles.container}>
      <canvas
        ref={canvasRef}
        id="map-canvas"
        className={styles.canvas}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
        onWheel={handleWheel}
        onContextMenu={(e) => {
          e.preventDefault();
          // Right-click is used for lowering in HeightRaise mode
          if (activeTool === Tool.HeightRaise) return;
          const rect = canvasRef.current?.getBoundingClientRect();
          if (!rect) return;
          const tile = screenToWorldTile(
            { sx: e.clientX - rect.left, sy: e.clientY - rect.top },
            viewport
          );
          if (tile) {
            setContextMenu({ x: e.clientX, y: e.clientY, wx: tile.wx, wy: tile.wy });
          }
        }}
      />
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={[
            {
              label: 'Add Note Here',
              onClick: () => {
                const now = new Date().toISOString();
                const note: DevNote = {
                  id: crypto.randomUUID(),
                  x: contextMenu.wx,
                  y: contextMenu.wy,
                  chunkCoord: worldToChunk({ wx: contextMenu.wx, wy: contextMenu.wy }),
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
                setShowNotes(true);
                setNotesPanelCollapsed(false);
              },
            },
          ]}
          onClose={() => setContextMenu(null)}
        />
      )}
      {activeTool === Tool.BlockType && (
        <div className={styles.modeIndicator} style={{ gap: '8px', pointerEvents: 'auto' }}>
          <span>Down:</span>
          <input
            type="number"
            min={0}
            max={65535}
            value={selectedBlockTypeDown}
            onChange={(e) => useEditorStore.getState().setSelectedBlockTypeDown(parseInt(e.target.value) || 0)}
            onKeyDown={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            style={{ width: '50px', padding: '2px 4px', background: '#333', color: '#fff', border: '1px solid #555', borderRadius: '3px' }}
          />
          <span>Right:</span>
          <input
            type="number"
            min={0}
            max={65535}
            value={selectedBlockTypeRight}
            onChange={(e) => useEditorStore.getState().setSelectedBlockTypeRight(parseInt(e.target.value) || 0)}
            onKeyDown={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            style={{ width: '50px', padding: '2px 4px', background: '#333', color: '#fff', border: '1px solid #555', borderRadius: '3px' }}
          />
          <span style={{ fontSize: '11px', opacity: 0.7 }}>(wall sprite IDs, 0 = plain)</span>
        </div>
      )}
      {editorMode === 'interior' && currentInterior && (
        <div className={styles.modeIndicator}>
          INTERIOR MODE
          <span className={styles.interiorName}>{currentInterior.name}</span>
        </div>
      )}
      {hoveredTile && (
        <div className={styles.coords}>
          {hoveredTile.wx}, {hoveredTile.wy}
          {(activeTool === Tool.HeightRaise || activeTool === Tool.BlockType) && (() => {
            const cc = worldToChunk(hoveredTile);
            const chunk = chunks.get(`${cc.cx},${cc.cy}`);
            const lx = ((hoveredTile.wx % 32) + 32) % 32;
            const ly = ((hoveredTile.wy % 32) + 32) % 32;
            const idx = ly * 32 + lx;
            const h = chunk?.heights ? chunk.heights[idx] : 0;
            const btD = chunk?.blockTypesDown ? chunk.blockTypesDown[idx] : 0;
            const btR = chunk?.blockTypesRight ? chunk.blockTypesRight[idx] : 0;
            if (activeTool === Tool.BlockType) {
              return ` (H: ${h}, D: ${btD}, R: ${btR})`;
            }
            return ` (H: ${h})`;
          })()}
        </div>
      )}
    </div>
  );
}
