import { useEditorStore } from '@/state/store';
import { useShallow } from 'zustand/react/shallow';
import { Layer } from '@/types';
import styles from './LayerPanel.module.css';

// Add a special 'portals' layer type for visibility toggle
type ExtendedLayer = Layer | 'portals' | 'gatheringZones' | 'farmingPlots';

const layers: { id: ExtendedLayer; label: string }[] = [
  { id: Layer.Ground, label: 'Ground' },
  { id: Layer.Objects, label: 'Objects (Tiles)' },
  { id: Layer.Overhead, label: 'Overhead' },
  { id: Layer.MapObjects, label: 'Map Objects' },
  { id: Layer.Collision, label: 'Collision' },
  { id: Layer.Entities, label: 'Entities' },
  { id: 'portals', label: 'Portals' },
  { id: 'gatheringZones', label: 'Gathering Zones' },
  { id: 'farmingPlots', label: 'Farming Plots' },
];

export function LayerPanel() {
  const {
    layerPanelCollapsed: collapsed,
    setLayerPanelCollapsed: setCollapsed,
    activeLayer,
    setActiveLayer,
    visibleLayers,
    setLayerVisibility,
    showCollision,
    showEntities,
    showMapObjects,
    showPortals,
    showGatheringZones,
    showFarmingPlots,
    toggleCollisionOverlay,
    toggleEntitiesOverlay,
    toggleMapObjectsOverlay,
    togglePortalsOverlay,
    toggleGatheringZonesOverlay,
    toggleFarmingPlotsOverlay,
  } = useEditorStore(
    useShallow((s) => ({
      layerPanelCollapsed: s.layerPanelCollapsed,
      setLayerPanelCollapsed: s.setLayerPanelCollapsed,
      activeLayer: s.activeLayer,
      setActiveLayer: s.setActiveLayer,
      visibleLayers: s.visibleLayers,
      setLayerVisibility: s.setLayerVisibility,
      showCollision: s.showCollision,
      showEntities: s.showEntities,
      showMapObjects: s.showMapObjects,
      showPortals: s.showPortals,
      showGatheringZones: s.showGatheringZones,
      showFarmingPlots: s.showFarmingPlots,
      toggleCollisionOverlay: s.toggleCollisionOverlay,
      toggleEntitiesOverlay: s.toggleEntitiesOverlay,
      toggleMapObjectsOverlay: s.toggleMapObjectsOverlay,
      togglePortalsOverlay: s.togglePortalsOverlay,
      toggleGatheringZonesOverlay: s.toggleGatheringZonesOverlay,
      toggleFarmingPlotsOverlay: s.toggleFarmingPlotsOverlay,
    })),
  );

  const isLayerVisible = (layer: ExtendedLayer): boolean => {
    switch (layer) {
      case Layer.Ground:
        return visibleLayers.ground;
      case Layer.Objects:
        return visibleLayers.objects;
      case Layer.Overhead:
        return visibleLayers.overhead;
      case Layer.Collision:
        return showCollision;
      case Layer.Entities:
        return showEntities;
      case Layer.MapObjects:
        return showMapObjects;
      case 'portals':
        return showPortals;
      case 'gatheringZones':
        return showGatheringZones;
      case 'farmingPlots':
        return showFarmingPlots;
      default:
        return true;
    }
  };

  const toggleVisibility = (layer: ExtendedLayer) => {
    switch (layer) {
      case Layer.Ground:
        setLayerVisibility('ground', !visibleLayers.ground);
        break;
      case Layer.Objects:
        setLayerVisibility('objects', !visibleLayers.objects);
        break;
      case Layer.Overhead:
        setLayerVisibility('overhead', !visibleLayers.overhead);
        break;
      case Layer.Collision:
        toggleCollisionOverlay();
        break;
      case Layer.Entities:
        toggleEntitiesOverlay();
        break;
      case Layer.MapObjects:
        toggleMapObjectsOverlay();
        break;
      case 'portals':
        togglePortalsOverlay();
        break;
      case 'gatheringZones':
        toggleGatheringZonesOverlay();
        break;
      case 'farmingPlots':
        toggleFarmingPlotsOverlay();
        break;
    }
  };

  const handleLayerClick = (layer: ExtendedLayer) => {
    // These are visibility-only toggles, not editable layers.
    if (layer !== 'portals' && layer !== 'gatheringZones' && layer !== 'farmingPlots') {
      setActiveLayer(layer as Layer);
    }
  };

  return (
    <div className={`${styles.panel} ${collapsed ? styles.collapsed : ''}`}>
      <button className={styles.header} onClick={() => setCollapsed(!collapsed)}>
        <span className={`${styles.arrow} ${collapsed ? styles.arrowCollapsed : ''}`}>&#9662;</span>
        <span className={styles.title}>Layers</span>
      </button>
      {!collapsed && (
        <div className={styles.layers}>
          {layers.map((layer) => (
            <div
              key={layer.id}
              className={`${styles.layer} ${activeLayer === layer.id ? styles.active : ''}`}
            >
              <input
                type="checkbox"
                className={styles.checkbox}
                checked={isLayerVisible(layer.id)}
                onChange={() => toggleVisibility(layer.id)}
              />
              <button
                className={styles.layerButton}
                onClick={() => handleLayerClick(layer.id)}
              >
                {layer.label}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
