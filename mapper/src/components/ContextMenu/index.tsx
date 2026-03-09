import styles from './ContextMenu.module.css';

interface MenuItem {
  label: string;
  onClick: () => void;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  return (
    <div className={styles.overlay} onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }}>
      <div className={styles.menu} style={{ left: x, top: y }}>
        {items.map((item, i) => (
          <button
            key={i}
            className={styles.item}
            onClick={() => { item.onClick(); onClose(); }}
          >
            {item.label}
          </button>
        ))}
      </div>
    </div>
  );
}
