import { useEditorStore } from '@/state/store';
import { Tool } from '@/types';
import {
  MousePointer2,
  Paintbrush,
  PaintBucket,
  WandSparkles,
  Eraser,
  Grid3x3,
  User,
  Box,
  DoorOpen,
  Pickaxe,
  Pipette,
  House,
  ArrowDown,
  ArrowRight,
  Flag,
  LogOut,
  Mountain,
  Layers,
  type LucideIcon,
} from 'lucide-react';
import styles from './Toolbar.module.css';

interface ToolDef {
  id: Tool;
  label: string;
  shortcut: string;
  icon: LucideIcon;
  group: 'main' | 'walls' | 'interior';
  mode?: 'overworld' | 'interior';
  composite?: { icon: LucideIcon };
}

const tools: ToolDef[] = [
  { id: Tool.Select, label: 'Select', shortcut: 'V', icon: MousePointer2, group: 'main' },
  { id: Tool.Paint, label: 'Paint', shortcut: 'B', icon: Paintbrush, group: 'main' },
  { id: Tool.Fill, label: 'Fill', shortcut: 'G', icon: PaintBucket, group: 'main' },
  { id: Tool.MagicWand, label: 'Magic Wand', shortcut: 'W', icon: WandSparkles, group: 'main' },
  { id: Tool.Eraser, label: 'Eraser', shortcut: 'E', icon: Eraser, group: 'main' },
  { id: Tool.Eyedropper, label: 'Eyedropper', shortcut: 'I', icon: Pipette, group: 'main' },
  { id: Tool.Collision, label: 'Collision', shortcut: 'C', icon: Grid3x3, group: 'main' },
  { id: Tool.Entity, label: 'Entity', shortcut: 'N', icon: User, group: 'main' },
  { id: Tool.Object, label: 'Object', shortcut: 'O', icon: Box, group: 'main' },
  { id: Tool.Portal, label: 'Portal', shortcut: 'P', icon: DoorOpen, group: 'main', mode: 'overworld' },
  { id: Tool.GatheringZone, label: 'Gathering', shortcut: 'F', icon: Pickaxe, group: 'main', mode: 'overworld' },
  { id: Tool.HeightRaise, label: 'Height', shortcut: 'H', icon: Mountain, group: 'main', mode: 'overworld' },
  { id: Tool.BlockType, label: 'Block Type', shortcut: 'T', icon: Layers, group: 'main', mode: 'overworld' },
  { id: Tool.WallDown, label: 'Wall Down', shortcut: 'D', icon: House, group: 'walls', composite: { icon: ArrowDown } },
  { id: Tool.WallRight, label: 'Wall Right', shortcut: 'R', icon: House, group: 'walls', composite: { icon: ArrowRight } },
  { id: Tool.SpawnPoint, label: 'Spawn Point', shortcut: 'S', icon: Flag, group: 'interior', mode: 'interior' },
  { id: Tool.ExitPortal, label: 'Exit Portal', shortcut: 'X', icon: LogOut, group: 'interior', mode: 'interior' },
];

export function Toolbar() {
  const { activeTool, setActiveTool, editorMode, brushSize, setBrushSize } = useEditorStore();

  const visibleTools = tools.filter((t) => !t.mode || t.mode === editorMode);

  const mainTools = visibleTools.filter((t) => t.group === 'main');
  const wallTools = visibleTools.filter((t) => t.group === 'walls');
  const interiorTools = visibleTools.filter((t) => t.group === 'interior');

  const renderButton = (tool: ToolDef) => {
    const Icon = tool.icon;
    const isActive = activeTool === tool.id;

    return (
      <button
        key={tool.id}
        className={`${styles.btn} ${isActive ? styles.active : ''}`}
        onClick={() => setActiveTool(tool.id)}
        data-tooltip={`${tool.label} (${tool.shortcut})`}
      >
        {tool.composite ? (
          <span className={styles.compositeIcon}>
            <Icon size={16} />
            <tool.composite.icon size={9} className={styles.compositeArrow} />
          </span>
        ) : (
          <Icon size={16} />
        )}
      </button>
    );
  };

  return (
    <div className={styles.toolbar}>
      {mainTools.map(renderButton)}
      {wallTools.length > 0 && (
        <>
          <div className={styles.separator} />
          {wallTools.map(renderButton)}
        </>
      )}
      {interiorTools.length > 0 && (
        <>
          <div className={styles.separator} />
          {interiorTools.map(renderButton)}
        </>
      )}
      <div className={styles.spacer} />
      <div className={styles.brushSize} data-tooltip={`Brush Size (Ctrl+Scroll)`}>
        <span className={styles.brushLabel}>{brushSize}</span>
        <div className={styles.brushControls}>
          <button
            className={styles.brushBtn}
            onClick={() => setBrushSize(brushSize - 1)}
            disabled={brushSize <= 1}
          >-</button>
          <button
            className={styles.brushBtn}
            onClick={() => setBrushSize(brushSize + 1)}
            disabled={brushSize >= 15}
          >+</button>
        </div>
      </div>
    </div>
  );
}
