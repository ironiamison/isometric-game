import { useEffect } from 'react';
import { useEditorStore } from '@/state/store';
import { objectLoader } from '@/core/ObjectLoader';
import { interiorStorage } from '@/core/InteriorStorage';
import type { EntitySpawn, Portal, ExitPortal, Wall } from '@/types';
import styles from './PropertiesPanel.module.css';

const GATHERING_ZONE_TYPES = [
  { id: 'pond', label: 'Pond (Fishing Lv 1)' },
  { id: 'river', label: 'River (Fishing Lv 15)' },
  { id: 'ocean', label: 'Ocean (Fishing Lv 40)' },
];

export function PropertiesPanel() {
  const {
    chunks,
    activeTool,
    selectedEntitySpawn,
    selectedMapObject,
    selectedPortal,
    selectedGatheringZone,
    selectedGatheringZoneId,
    selectedExitPortal,
    currentInterior,
    availableInteriors,
    setSelectedEntitySpawn,
    setSelectedMapObject,
    setSelectedPortal,
    setSelectedGatheringZone,
    setSelectedGatheringZoneId,
    setSelectedExitPortal,
    updateEntity,
    removeEntity,
    removeMapObject,
    updateMapObject,
    updatePortal,
    removePortal,
    removeGatheringZone,
    updateGatheringZone,
    updateExitPortal,
    removeExitPortal,
    jumpToPortalTarget,
    jumpToExitPortalTarget,
    editorMode,
    selectedInteriorEntity,
    selectedInteriorMapObject,
    selectedInteriorWall,
    selectedWall,
    setSelectedInteriorEntity,
    setSelectedInteriorMapObject,
    setSelectedInteriorWall,
    setSelectedWall,
    removeInteriorEntity,
    removeInteriorMapObject,
    removeInteriorWall,
    removeWall,
    updateInteriorEntity,
  } = useEditorStore();

  // Get the actual entity spawn from the selection
  const getSelectedEntity = () => {
    if (!selectedEntitySpawn) return null;
    const chunk = chunks.get(
      `${selectedEntitySpawn.chunkCoord.cx},${selectedEntitySpawn.chunkCoord.cy}`
    );
    if (!chunk) return null;
    const spawn = chunk.entities.find((e) => e.id === selectedEntitySpawn.spawnId);
    if (!spawn) return null;
    return { spawn, chunkCoord: selectedEntitySpawn.chunkCoord };
  };

  // Get the actual map object from the selection
  const getSelectedObject = () => {
    if (!selectedMapObject) return null;
    const chunk = chunks.get(
      `${selectedMapObject.chunkCoord.cx},${selectedMapObject.chunkCoord.cy}`
    );
    if (!chunk) return null;
    const obj = chunk.mapObjects.find((o) => o.id === selectedMapObject.objectId);
    if (!obj) return null;
    return { object: obj, chunkCoord: selectedMapObject.chunkCoord };
  };

  // Get the actual portal from the selection
  const getSelectedPortalData = () => {
    if (!selectedPortal) return null;
    const chunk = chunks.get(
      `${selectedPortal.chunkCoord.cx},${selectedPortal.chunkCoord.cy}`
    );
    if (!chunk || !chunk.portals) return null;
    const portal = chunk.portals.find((p) => p.id === selectedPortal.portalId);
    if (!portal) return null;
    return { portal, chunkCoord: selectedPortal.chunkCoord };
  };

  // Get the actual exit portal from the selection (interior mode)
  const getSelectedExitPortalData = () => {
    if (!selectedExitPortal || !currentInterior) return null;
    const portal = currentInterior.exitPortals?.find((p) => p.id === selectedExitPortal.portalId);
    if (!portal) return null;
    return portal;
  };

  // Get the actual gathering zone from the selection
  const getSelectedGatheringZoneData = () => {
    if (!selectedGatheringZone) return null;
    const chunk = chunks.get(
      `${selectedGatheringZone.chunkCoord.cx},${selectedGatheringZone.chunkCoord.cy}`
    );
    if (!chunk || !chunk.gatheringZones) return null;
    const zone = chunk.gatheringZones.find((g) => g.id === selectedGatheringZone.zoneId);
    if (!zone) return null;
    return { zone, chunkCoord: selectedGatheringZone.chunkCoord };
  };

  // Get selected interior entity from currentInterior
  const getSelectedInteriorEntityData = () => {
    if (!selectedInteriorEntity || !currentInterior) return null;
    return currentInterior.entities.find((e) => e.id === selectedInteriorEntity) || null;
  };

  // Get selected interior map object from currentInterior
  const getSelectedInteriorMapObjectData = () => {
    if (!selectedInteriorMapObject || !currentInterior) return null;
    return currentInterior.mapObjects.find((o) => o.id === selectedInteriorMapObject) || null;
  };

  // Get selected interior wall from currentInterior
  const getSelectedInteriorWallData = () => {
    if (!selectedInteriorWall || !currentInterior) return null;
    return currentInterior.walls.find((w) => w.id === selectedInteriorWall) || null;
  };

  // Get the actual overworld wall from the selection
  const getSelectedWallData = () => {
    if (!selectedWall) return null;
    const chunk = chunks.get(
      `${selectedWall.chunkCoord.cx},${selectedWall.chunkCoord.cy}`
    );
    if (!chunk) return null;
    const wall = chunk.walls.find((w) => w.id === selectedWall.wallId);
    if (!wall) return null;
    return { wall, chunkCoord: selectedWall.chunkCoord };
  };

  const selectedEntity = getSelectedEntity();
  const selectedObject = getSelectedObject();
  const selectedWallData = getSelectedWallData();
  const selectedPortalData = getSelectedPortalData();
  const selectedExitPortalData = getSelectedExitPortalData();
  const selectedGatheringZoneData = getSelectedGatheringZoneData();
  const interiorEntityData = getSelectedInteriorEntityData();
  const interiorObjectData = getSelectedInteriorMapObjectData();
  const interiorWallData = getSelectedInteriorWallData();

  // Delete key handler
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      if (e.key === 'Delete' || e.key === 'Backspace') {
        if (selectedEntity) {
          e.preventDefault();
          removeEntity(selectedEntity.chunkCoord, selectedEntity.spawn.id);
          setSelectedEntitySpawn(null);
        } else if (selectedObject) {
          e.preventDefault();
          removeMapObject(selectedObject.chunkCoord, selectedObject.object.id);
          setSelectedMapObject(null);
        } else if (selectedPortalData) {
          e.preventDefault();
          removePortal(selectedPortalData.chunkCoord, selectedPortalData.portal.id);
          setSelectedPortal(null);
        } else if (selectedGatheringZoneData) {
          e.preventDefault();
          removeGatheringZone(selectedGatheringZoneData.chunkCoord, selectedGatheringZoneData.zone.id);
          setSelectedGatheringZone(null);
        } else if (selectedExitPortalData) {
          e.preventDefault();
          removeExitPortal(selectedExitPortalData.id);
          setSelectedExitPortal(null);
        } else if (interiorEntityData) {
          e.preventDefault();
          removeInteriorEntity(interiorEntityData.id);
          setSelectedInteriorEntity(null);
        } else if (interiorObjectData) {
          e.preventDefault();
          removeInteriorMapObject(interiorObjectData.id);
          setSelectedInteriorMapObject(null);
        } else if (interiorWallData) {
          e.preventDefault();
          removeInteriorWall(interiorWallData.id);
          setSelectedInteriorWall(null);
        } else if (selectedWallData) {
          e.preventDefault();
          removeWall(selectedWallData.chunkCoord, selectedWallData.wall.id);
          setSelectedWall(null);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedEntity, selectedObject, selectedWallData, selectedPortalData, selectedGatheringZoneData, selectedExitPortalData, interiorEntityData, interiorObjectData, interiorWallData, removeEntity, removeMapObject, removeWall, removePortal, removeGatheringZone, removeExitPortal, removeInteriorEntity, removeInteriorMapObject, removeInteriorWall, setSelectedEntitySpawn, setSelectedMapObject, setSelectedWall, setSelectedPortal, setSelectedGatheringZone, setSelectedExitPortal, setSelectedInteriorEntity, setSelectedInteriorMapObject, setSelectedInteriorWall]);

  // Show portal properties
  if (selectedPortalData) {
    const { portal, chunkCoord } = selectedPortalData;

    const handlePortalChange = (field: keyof Portal, value: string | number) => {
      updatePortal(chunkCoord, portal.id, { [field]: value });
    };

    const handleDeletePortal = () => {
      removePortal(chunkCoord, portal.id);
      setSelectedPortal(null);
    };

    const handleJumpToTarget = () => {
      if (portal.targetMap) {
        jumpToPortalTarget(portal);
      }
    };

    // Get spawn points for the selected target map
    const targetSpawnPoints = portal.targetMap
      ? interiorStorage.getSpawnPoints(portal.targetMap)
      : [];

    // Calculate world position
    const worldX = chunkCoord.cx * 32 + portal.x;
    const worldY = chunkCoord.cy * 32 + portal.y;

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Portal Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Target Map</label>
            {availableInteriors.length > 0 ? (
              <select
                className={styles.select}
                value={portal.targetMap}
                onChange={(e) => handlePortalChange('targetMap', e.target.value)}
              >
                <option value="">Select interior...</option>
                {availableInteriors.map((id) => (
                  <option key={id} value={id}>{id}</option>
                ))}
              </select>
            ) : (
              <input
                type="text"
                className={styles.input}
                value={portal.targetMap}
                placeholder="e.g., test_house"
                onChange={(e) => handlePortalChange('targetMap', e.target.value)}
              />
            )}
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Target Spawn</label>
            {targetSpawnPoints.length > 0 ? (
              <select
                className={styles.select}
                value={portal.targetSpawn}
                onChange={(e) => handlePortalChange('targetSpawn', e.target.value)}
              >
                <option value="">Select spawn point...</option>
                {targetSpawnPoints.map((sp) => (
                  <option key={sp.name} value={sp.name}>{sp.name}</option>
                ))}
              </select>
            ) : (
              <input
                type="text"
                className={styles.input}
                value={portal.targetSpawn}
                placeholder="e.g., entrance"
                onChange={(e) => handlePortalChange('targetSpawn', e.target.value)}
              />
            )}
          </div>

          {portal.targetMap && (
            <button
              className={styles.jumpButton}
              onClick={handleJumpToTarget}
            >
              Jump To Target
            </button>
          )}

          <div className={styles.field}>
            <label className={styles.label}>Width (tiles)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.width}
              min={1}
              onChange={(e) => handlePortalChange('width', Math.max(1, parseInt(e.target.value) || 1))}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Height (tiles)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.height}
              min={1}
              onChange={(e) => handlePortalChange('height', Math.max(1, parseInt(e.target.value) || 1))}
            />
          </div>

          <div className={styles.info}>
            Position: {worldX}, {worldY}
            <br />
            Chunk: {chunkCoord.cx}, {chunkCoord.cy}
            <br />
            Local: {portal.x}, {portal.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDeletePortal}>
              Delete Portal
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show exit portal properties (interior mode)
  if (selectedExitPortalData && currentInterior) {
    const portal = selectedExitPortalData;

    const handleExitPortalChange = (field: keyof ExitPortal, value: number) => {
      updateExitPortal(portal.id, { [field]: value });
    };

    const handleDeleteExitPortal = () => {
      removeExitPortal(portal.id);
      setSelectedExitPortal(null);
    };

    const handleJumpToTarget = () => {
      jumpToExitPortalTarget(portal);
    };

    const handleApplyToAll = () => {
      // Apply current targetX/targetY to all exit portals in this interior
      const exitPortals = currentInterior.exitPortals || [];
      for (const p of exitPortals) {
        updateExitPortal(p.id, { targetX: portal.targetX, targetY: portal.targetY });
      }
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Exit Portal Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Exit Target X (world)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.targetX}
              onChange={(e) => handleExitPortalChange('targetX', parseFloat(e.target.value) || 0)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Exit Target Y (world)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.targetY}
              onChange={(e) => handleExitPortalChange('targetY', parseFloat(e.target.value) || 0)}
            />
          </div>

          {(portal.targetX !== 0 || portal.targetY !== 0) && (
            <button
              className={styles.jumpButton}
              onClick={handleJumpToTarget}
            >
              Jump To Exit Location
            </button>
          )}

          <button
            className={styles.applyAllButton}
            onClick={handleApplyToAll}
          >
            Apply to All Exits ({currentInterior.exitPortals?.length || 0})
          </button>

          <div className={styles.field}>
            <label className={styles.label}>Width (tiles)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.width}
              min={1}
              onChange={(e) => handleExitPortalChange('width', Math.max(1, parseInt(e.target.value) || 1))}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Height (tiles)</label>
            <input
              type="number"
              className={styles.input}
              value={portal.height}
              min={1}
              onChange={(e) => handleExitPortalChange('height', Math.max(1, parseInt(e.target.value) || 1))}
            />
          </div>

          <div className={styles.info}>
            Position in interior: {portal.x}, {portal.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDeleteExitPortal}>
              Delete Exit Portal
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show gathering zone properties
  if (selectedGatheringZoneData) {
    const { zone, chunkCoord } = selectedGatheringZoneData;

    const worldX = chunkCoord.cx * 32 + zone.x;
    const worldY = chunkCoord.cy * 32 + zone.y;

    const handleZoneTypeChange = (newZoneId: string) => {
      updateGatheringZone(chunkCoord, zone.id, { zoneId: newZoneId });
    };

    const handleDeleteZone = () => {
      removeGatheringZone(chunkCoord, zone.id);
      setSelectedGatheringZone(null);
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Gathering Zone</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Zone Type</label>
            <select
              className={styles.select}
              value={zone.zoneId}
              onChange={(e) => handleZoneTypeChange(e.target.value)}
            >
              {GATHERING_ZONE_TYPES.map((zt) => (
                <option key={zt.id} value={zt.id}>{zt.label}</option>
              ))}
            </select>
          </div>

          <div className={styles.info}>
            Position: {worldX}, {worldY}
            <br />
            Chunk: {chunkCoord.cx}, {chunkCoord.cy}
            <br />
            Local: {zone.x}, {zone.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDeleteZone}>
              Delete Zone
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show entity properties
  if (selectedEntity) {
    const { spawn, chunkCoord } = selectedEntity;

    const handleChange = (field: keyof EntitySpawn, value: string | number | boolean) => {
      updateEntity(chunkCoord, spawn.id, { [field]: value });
    };

    const handleDelete = () => {
      removeEntity(chunkCoord, spawn.id);
      setSelectedEntitySpawn(null);
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Entity Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Entity ID</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.entityId}
              onChange={(e) => handleChange('entityId', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Name</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.name}
              onChange={(e) => handleChange('name', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Level</label>
            <input
              type="number"
              className={styles.input}
              value={spawn.level}
              min={1}
              onChange={(e) => handleChange('level', parseInt(e.target.value) || 1)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Unique ID</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.uniqueId || ''}
              placeholder="Optional"
              onChange={(e) => handleChange('uniqueId', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Facing</label>
            <select
              className={styles.select}
              value={spawn.facing || ''}
              onChange={(e) => handleChange('facing', e.target.value)}
            >
              <option value="">Default</option>
              <option value="north">North</option>
              <option value="south">South</option>
              <option value="east">East</option>
              <option value="west">West</option>
            </select>
          </div>

          <div className={styles.field}>
            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={spawn.respawn ?? false}
                onChange={(e) => handleChange('respawn', e.target.checked)}
              />
              Respawn
            </label>
          </div>

          <div className={styles.info}>
            Position: {spawn.x}, {spawn.y}
            <br />
            Chunk: {chunkCoord.cx}, {chunkCoord.cy}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Entity
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show object properties
  if (selectedObject) {
    const { object, chunkCoord } = selectedObject;
    const objDef = objectLoader.getObject(objectLoader.gidToId(object.gid));

    const handleDelete = () => {
      removeMapObject(chunkCoord, object.id);
      setSelectedMapObject(null);
    };

    const handleToggleCollision = (noCollision: boolean) => {
      updateMapObject(chunkCoord, object.id, { noCollision });
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Object Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Type</label>
            <div className={styles.value}>{objDef?.name || `GID: ${object.gid}`}</div>
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Size</label>
            <div className={styles.value}>{object.width} x {object.height} px</div>
          </div>

          <div className={styles.field}>
            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={object.noCollision ?? false}
                onChange={(e) => handleToggleCollision(e.target.checked)}
              />
              No Collision
            </label>
          </div>

          <div className={styles.info}>
            Position: {object.x}, {object.y}
            <br />
            Chunk: {chunkCoord.cx}, {chunkCoord.cy}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Object
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show interior entity properties
  if (interiorEntityData && editorMode === 'interior') {
    const spawn = interiorEntityData;

    const handleChange = (field: keyof EntitySpawn, value: string | number | boolean) => {
      updateInteriorEntity(spawn.id, { [field]: value });
    };

    const handleDelete = () => {
      removeInteriorEntity(spawn.id);
      setSelectedInteriorEntity(null);
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Interior Entity Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Entity ID</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.entityId}
              onChange={(e) => handleChange('entityId', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Name</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.name}
              onChange={(e) => handleChange('name', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Level</label>
            <input
              type="number"
              className={styles.input}
              value={spawn.level}
              min={1}
              onChange={(e) => handleChange('level', parseInt(e.target.value) || 1)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Unique ID</label>
            <input
              type="text"
              className={styles.input}
              value={spawn.uniqueId || ''}
              placeholder="Optional"
              onChange={(e) => handleChange('uniqueId', e.target.value)}
            />
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Facing</label>
            <select
              className={styles.select}
              value={spawn.facing || ''}
              onChange={(e) => handleChange('facing', e.target.value)}
            >
              <option value="">Default</option>
              <option value="north">North</option>
              <option value="south">South</option>
              <option value="east">East</option>
              <option value="west">West</option>
            </select>
          </div>

          <div className={styles.field}>
            <label className={styles.checkboxLabel}>
              <input
                type="checkbox"
                checked={spawn.respawn ?? false}
                onChange={(e) => handleChange('respawn', e.target.checked)}
              />
              Respawn
            </label>
          </div>

          <div className={styles.info}>
            Position: {spawn.x}, {spawn.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Entity
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show interior object properties
  if (interiorObjectData && editorMode === 'interior') {
    const object = interiorObjectData;
    const objDef = objectLoader.getObject(objectLoader.gidToId(object.gid));

    const handleDelete = () => {
      removeInteriorMapObject(object.id);
      setSelectedInteriorMapObject(null);
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Interior Object Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Type</label>
            <div className={styles.value}>{objDef?.name || `GID: ${object.gid}`}</div>
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Size</label>
            <div className={styles.value}>{object.width} x {object.height} px</div>
          </div>

          <div className={styles.info}>
            Position: {object.x}, {object.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Object
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show interior wall properties
  if (interiorWallData && editorMode === 'interior') {
    const wall = interiorWallData;
    const wallDef = objectLoader.getWallByGid(wall.gid);

    const handleDelete = () => {
      removeInteriorWall(wall.id);
      setSelectedInteriorWall(null);
    };

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Interior Wall Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Type</label>
            <div className={styles.value}>{wallDef?.name || `GID: ${wall.gid}`}</div>
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Edge</label>
            <div className={styles.value}>{wall.edge}</div>
          </div>

          <div className={styles.info}>
            Position: {wall.x}, {wall.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Wall
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show overworld wall properties
  if (selectedWallData) {
    const { wall, chunkCoord } = selectedWallData;
    const wallDef = objectLoader.getWallByGid(wall.gid);

    const handleDelete = () => {
      removeWall(chunkCoord, wall.id);
      setSelectedWall(null);
    };

    const worldX = chunkCoord.cx * 32 + wall.x;
    const worldY = chunkCoord.cy * 32 + wall.y;

    return (
      <div className={styles.panel}>
        <div className={styles.title}>Wall Properties</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Type</label>
            <div className={styles.value}>{wallDef?.name || `GID: ${wall.gid}`}</div>
          </div>

          <div className={styles.field}>
            <label className={styles.label}>Edge</label>
            <div className={styles.value}>{wall.edge}</div>
          </div>

          <div className={styles.info}>
            Position: {worldX}, {worldY}
            <br />
            Chunk: {chunkCoord.cx}, {chunkCoord.cy}
            <br />
            Local: {wall.x}, {wall.y}
          </div>

          <div className={styles.actions}>
            <button className={styles.deleteButton} onClick={handleDelete}>
              Delete Wall
            </button>
          </div>
          <div className={styles.hint}>Press Delete to remove</div>
        </div>
      </div>
    );
  }

  // Show zone type selector when gathering tool is active
  if (activeTool === 'gatheringZone') {
    return (
      <div className={styles.panel}>
        <div className={styles.title}>Gathering Zone</div>
        <div className={styles.content}>
          <div className={styles.field}>
            <label className={styles.label}>Place Zone Type</label>
            <select
              className={styles.select}
              value={selectedGatheringZoneId}
              onChange={(e) => setSelectedGatheringZoneId(e.target.value)}
            >
              {GATHERING_ZONE_TYPES.map((zt) => (
                <option key={zt.id} value={zt.id}>{zt.label}</option>
              ))}
            </select>
          </div>
          <div className={styles.hint}>Click tiles to place zones. Click existing zones to edit.</div>
        </div>
      </div>
    );
  }

  // Nothing selected - hide the panel entirely
  return null;
}
