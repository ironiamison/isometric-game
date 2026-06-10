import { useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  BarChart3,
  Boxes,
  ChevronLeft,
  Map as MapIcon,
  Package,
  Plus,
  RefreshCw,
  Save,
  Search,
  Shield,
  Sparkles,
  Swords,
  Trash2,
} from 'lucide-react';
import { chunkManager } from '@/core/ChunkManager';
import { chunkKey } from '@/core/coords';
import { useEditorStore } from '@/state/store';
import type { Chunk } from '@/types';
import { deleteContentEntry, loadContentCatalog, saveContentEntry } from '@/content/api';
import { calculateEnemyBalance, equipmentPower, playerMaxHit } from '@/content/balance';
import type {
  ContentCatalog,
  ContentData,
  ContentEntry,
  ContentKind,
  PlayerBalanceProfile,
} from '@/content/types';
import { validateContent } from '@/content/validation';
import styles from './ContentStudio.module.css';

type StudioTab = 'overview' | 'item' | 'enemy' | 'attack' | 'balance' | 'maps';

interface ContentStudioProps {
  onOpenMap: () => void;
}

interface FieldProps {
  label: string;
  value: string | number;
  onChange: (value: string) => void;
  type?: 'text' | 'number';
  min?: number;
  step?: number;
  help?: string;
}

function Field({ label, value, onChange, type = 'text', min, step, help }: FieldProps) {
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <input
        type={type}
        value={value}
        min={min}
        step={step}
        onChange={(event) => onChange(event.target.value)}
      />
      {help && <small>{help}</small>}
    </label>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: string[];
  onChange: (value: string) => void;
}) {
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => <option key={option} value={option}>{option}</option>)}
      </select>
    </label>
  );
}

function CheckboxField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className={styles.checkboxField}>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className={styles.formSection}>
      <h3>{title}</h3>
      <div className={styles.fieldGrid}>{children}</div>
    </section>
  );
}

function asRecord(value: unknown): ContentData {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as ContentData
    : {};
}

function asNumber(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function asBoolean(value: unknown, fallback = false): boolean {
  return typeof value === 'boolean' ? value : fallback;
}

function flattenCatalog(catalog: ContentCatalog | null): ContentEntry[] {
  if (!catalog) return [];
  return catalog.files.flatMap((file) => Object.entries(file.entries).map(([id, data]) => ({
    id,
    kind: file.kind,
    file: file.path,
    data: asRecord(data),
  })));
}

function defaultEntry(kind: ContentKind): ContentData {
  if (kind === 'item') {
    return {
      display_name: 'New Item',
      sprite: 'new_item',
      description: '',
      category: 'material',
      max_stack: 99,
      base_price: 1,
      sellable: true,
    };
  }
  if (kind === 'enemy') {
    return {
      display_name: 'New Enemy',
      sprite: 'new_enemy',
      animation_type: 'standard',
      description: '',
      stats: {
        level: 1,
        max_hp: 10,
        damage: 1,
        attack_bonus: 0,
        defence_bonus: 0,
        attack_range: 1,
        aggro_range: 4,
        chase_range: 6,
        move_cooldown_ms: 600,
        attack_cooldown_ms: 2000,
        respawn_time_ms: 10000,
      },
      rewards: { exp_base: 10, gold_min: 1, gold_max: 5 },
      behaviors: {
        hostile: true,
        wander_enabled: true,
        wander_radius: 3,
        wander_pause_min_ms: 2000,
        wander_pause_max_ms: 5000,
      },
      loot: [],
    };
  }
  return {
    name: 'New Attack',
    spell_type: 'damage',
    mana_cost: 5,
    cooldown_ms: 2000,
    base_power: 5,
    effect_sprite: 'fire_blast',
    description: '',
  };
}

function defaultUseEffect(type: string): ContentData {
  switch (type) {
    case 'heal':
    case 'restore_mana':
    case 'restore_prayer':
      return { type, amount: 1 };
    case 'buff':
      return { type, stat: 'attack', amount: 1, duration_ms: 60000 };
    case 'teleport':
      return { type, destination: 'overworld', x: 0, y: 0 };
    case 'learn_spell':
      return { type, spell_id: '' };
    case 'open_crate':
      return { type, tier: 'artisan', bracket: 'low' };
    case 'dig':
      return { type };
    default:
      return {};
  }
}

function spriteUrl(entry: ContentEntry): string | null {
  const sprite = String(entry.data.sprite || entry.data.effect_sprite || '');
  if (!sprite) return null;
  const folder = entry.kind === 'item'
    ? 'inventory'
    : entry.kind === 'attack' ? 'effects' : 'enemies';
  return `/mapper/assets/sprites/${folder}/${sprite}.png`;
}

export function ContentStudio({ onOpenMap }: ContentStudioProps) {
  const [catalog, setCatalog] = useState<ContentCatalog | null>(null);
  const [activeTab, setActiveTab] = useState<StudioTab>('overview');
  const [search, setSearch] = useState('');
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [draft, setDraft] = useState<ContentData | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [newId, setNewId] = useState('');
  const [newFile, setNewFile] = useState('');
  const [notice, setNotice] = useState('');
  const chunks = useEditorStore((state) => state.chunks);
  const setChunks = useEditorStore((state) => state.setChunks);
  const setWorldBounds = useEditorStore((state) => state.setWorldBounds);
  const currentWorld = useEditorStore((state) => state.currentWorld);

  const entries = useMemo(() => flattenCatalog(catalog), [catalog]);
  const selectedEntry = useMemo(
    () => entries.find((entry) => `${entry.file}:${entry.id}` === selectedKey) || null,
    [entries, selectedKey]
  );
  const issues = useMemo(
    () => validateContent(entries, chunks.values()),
    [entries, chunks]
  );

  const refresh = async (preferredKey?: string) => {
    setLoading(true);
    setError('');
    try {
      const nextCatalog = await loadContentCatalog();
      setCatalog(nextCatalog);
      if (preferredKey) setSelectedKey(preferredKey);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    setDraft(selectedEntry ? structuredClone(selectedEntry.data) : null);
  }, [selectedEntry]);

  const updateDraft = (path: string[], value: unknown) => {
    setDraft((current) => {
      if (!current) return current;
      const next = structuredClone(current);
      let target: ContentData = next;
      for (const key of path.slice(0, -1)) {
        target[key] = asRecord(target[key]);
        target = target[key] as ContentData;
      }
      target[path[path.length - 1]] = value;
      return next;
    });
  };

  const removeDraftPath = (path: string[]) => {
    setDraft((current) => {
      if (!current) return current;
      const next = structuredClone(current);
      let target: ContentData = next;
      for (const key of path.slice(0, -1)) target = asRecord(target[key]);
      delete target[path[path.length - 1]];
      return next;
    });
  };

  const handleSave = async () => {
    if (!selectedEntry || !draft) return;
    setSaving(true);
    setError('');
    try {
      await saveContentEntry(selectedEntry.file, selectedEntry.id, draft);
      setNotice(`Saved ${selectedEntry.id}`);
      await refresh(`${selectedEntry.file}:${selectedEntry.id}`);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!selectedEntry) return;
    if (!window.confirm(`Delete "${selectedEntry.id}" from ${selectedEntry.file}?`)) return;
    setSaving(true);
    try {
      await deleteContentEntry(selectedEntry.file, selectedEntry.id);
      setSelectedKey(null);
      setNotice(`Deleted ${selectedEntry.id}`);
      await refresh();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const openCreate = (kind: ContentKind) => {
    const firstFile = catalog?.files.find((file) => file.kind === kind)?.path || '';
    setNewId('');
    setNewFile(firstFile);
    setShowCreate(true);
  };

  const handleCreate = async () => {
    if (!newId || !newFile) return;
    if (!/^[a-z][a-z0-9_]*$/.test(newId)) {
      setError('IDs must be lowercase snake_case and start with a letter.');
      return;
    }
    const kind = catalog?.files.find((file) => file.path === newFile)?.kind;
    if (!kind) return;
    if (entries.some((entry) => entry.id === newId)) {
      setError(`The ID "${newId}" already exists.`);
      return;
    }

    setSaving(true);
    try {
      await saveContentEntry(newFile, newId, defaultEntry(kind));
      setShowCreate(false);
      setActiveTab(kind === 'npc' ? 'enemy' : kind);
      await refresh(`${newFile}:${newId}`);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const editorKind = activeTab === 'item' || activeTab === 'enemy' || activeTab === 'attack'
    ? activeTab
    : null;
  const listEntries = entries.filter((entry) => {
    if (editorKind && entry.kind !== editorKind) return false;
    const haystack = `${entry.id} ${String(entry.data.display_name || entry.data.name || '')}`.toLowerCase();
    return haystack.includes(search.toLowerCase());
  });

  const tabs: Array<{ id: StudioTab; label: string; icon: React.ReactNode }> = [
    { id: 'overview', label: 'Overview', icon: <Boxes size={16} /> },
    { id: 'item', label: 'Items', icon: <Package size={16} /> },
    { id: 'enemy', label: 'Enemies', icon: <Shield size={16} /> },
    { id: 'attack', label: 'Attacks', icon: <Sparkles size={16} /> },
    { id: 'balance', label: 'Balance Lab', icon: <BarChart3 size={16} /> },
    { id: 'maps', label: 'Map Tools', icon: <MapIcon size={16} /> },
  ];

  return (
    <div className={styles.studio}>
      <header className={styles.header}>
        <div>
          <button className={styles.backButton} onClick={onOpenMap}>
            <ChevronLeft size={16} /> Map Editor
          </button>
          <h1>Content Studio</h1>
          <p>Edit game data, compare balance, and catch broken references before launch.</p>
        </div>
        <div className={styles.headerActions}>
          <button onClick={() => void refresh()} disabled={loading}>
            <RefreshCw size={15} /> Reload
          </button>
          <span className={issues.some((issue) => issue.severity === 'error') ? styles.healthBad : styles.healthGood}>
            {issues.filter((issue) => issue.severity === 'error').length} errors
          </span>
        </div>
      </header>

      <nav className={styles.tabs}>
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={activeTab === tab.id ? styles.activeTab : ''}
            onClick={() => {
              setActiveTab(tab.id);
              setSelectedKey(null);
              setSearch('');
            }}
          >
            {tab.icon}{tab.label}
          </button>
        ))}
      </nav>

      {error && <div className={styles.errorBanner}>{error}</div>}
      {notice && <button className={styles.notice} onClick={() => setNotice('')}>{notice}</button>}

      <main className={styles.body}>
        {loading && !catalog ? (
          <div className={styles.emptyState}>Loading content files...</div>
        ) : activeTab === 'overview' ? (
          <Overview entries={entries} issues={issues} setActiveTab={setActiveTab} />
        ) : activeTab === 'balance' ? (
          <BalanceLab entries={entries} />
        ) : activeTab === 'maps' ? (
          <MapTools
            entries={entries}
            issues={issues}
            chunks={chunks}
            currentWorld={currentWorld}
            setChunks={setChunks}
            setWorldBounds={setWorldBounds}
            onOpenMap={onOpenMap}
            setNotice={setNotice}
          />
        ) : (
          <div className={styles.editorLayout}>
            <aside className={styles.entryList}>
              <div className={styles.listToolbar}>
                <label>
                  <Search size={14} />
                  <input
                    value={search}
                    onChange={(event) => setSearch(event.target.value)}
                    placeholder={`Search ${activeTab}...`}
                  />
                </label>
                <button onClick={() => openCreate(activeTab as ContentKind)} title="Create entry">
                  <Plus size={16} />
                </button>
              </div>
              <div className={styles.listCount}>{listEntries.length} definitions</div>
              <div className={styles.listScroll}>
                {listEntries.map((entry) => {
                  const key = `${entry.file}:${entry.id}`;
                  const image = spriteUrl(entry);
                  return (
                    <button
                      key={key}
                      className={`${styles.entryRow} ${selectedKey === key ? styles.selectedEntry : ''}`}
                      onClick={() => setSelectedKey(key)}
                    >
                      <span className={styles.spriteBox}>
                        {image ? <img src={image} alt="" /> : <Package size={17} />}
                      </span>
                      <span>
                        <strong>{String(entry.data.display_name || entry.data.name || entry.id)}</strong>
                        <small>{entry.id}</small>
                      </span>
                    </button>
                  );
                })}
              </div>
            </aside>

            <section className={styles.editorPanel}>
              {selectedEntry && draft ? (
                <>
                  <div className={styles.editorHeader}>
                    <div>
                      <span>{selectedEntry.file}</span>
                      <h2>{selectedEntry.id}</h2>
                    </div>
                    <div>
                      <button className={styles.deleteButton} onClick={() => void handleDelete()} disabled={saving}>
                        <Trash2 size={15} /> Delete
                      </button>
                      <button className={styles.primaryButton} onClick={() => void handleSave()} disabled={saving}>
                        <Save size={15} /> {saving ? 'Saving...' : 'Save'}
                      </button>
                    </div>
                  </div>
                  <div className={styles.formScroll}>
                    {selectedEntry.kind === 'item' && (
                      <ItemEditor draft={draft} update={updateDraft} remove={removeDraftPath} />
                    )}
                    {selectedEntry.kind === 'enemy' && (
                      <EnemyEditor draft={draft} update={updateDraft} />
                    )}
                    {selectedEntry.kind === 'attack' && (
                      <AttackEditor draft={draft} update={updateDraft} />
                    )}
                  </div>
                </>
              ) : (
                <div className={styles.emptyState}>Select a definition to edit it.</div>
              )}
            </section>
          </div>
        )}
      </main>

      {showCreate && (
        <div className={styles.modalOverlay}>
          <div className={styles.modal}>
            <h2>Create definition</h2>
            <Field label="ID" value={newId} onChange={setNewId} help="Lowercase snake_case, used by code and maps." />
            <SelectField
              label="File"
              value={newFile}
              options={(catalog?.files || [])
                .filter((file) => file.kind === activeTab)
                .map((file) => file.path)}
              onChange={setNewFile}
            />
            <div className={styles.modalActions}>
              <button onClick={() => setShowCreate(false)}>Cancel</button>
              <button className={styles.primaryButton} onClick={() => void handleCreate()} disabled={saving}>
                Create
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Overview({
  entries,
  issues,
  setActiveTab,
}: {
  entries: ContentEntry[];
  issues: ReturnType<typeof validateContent>;
  setActiveTab: (tab: StudioTab) => void;
}) {
  const cards: Array<{ kind: ContentKind; tab: StudioTab; label: string; icon: React.ReactNode }> = [
    { kind: 'item', tab: 'item', label: 'Items', icon: <Package size={22} /> },
    { kind: 'enemy', tab: 'enemy', label: 'Enemies', icon: <Shield size={22} /> },
    { kind: 'attack', tab: 'attack', label: 'Attacks', icon: <Sparkles size={22} /> },
    { kind: 'npc', tab: 'maps', label: 'NPC definitions', icon: <MapIcon size={22} /> },
  ];
  return (
    <div className={styles.dashboard}>
      <section className={styles.summaryGrid}>
        {cards.map((card) => (
          <button key={card.kind} className={styles.summaryCard} onClick={() => setActiveTab(card.tab)}>
            {card.icon}
            <span>{entries.filter((entry) => entry.kind === card.kind).length}</span>
            <small>{card.label}</small>
          </button>
        ))}
      </section>
      <section className={styles.issuePanel}>
        <div className={styles.panelTitle}>
          <div><AlertTriangle size={18} /><h2>Content Health</h2></div>
          <span>{issues.length} findings</span>
        </div>
        {issues.length === 0 ? (
          <div className={styles.successState}>No content or map validation issues found.</div>
        ) : (
          <div className={styles.issueList}>
            {issues.slice(0, 80).map((issue, index) => (
              <div key={`${issue.area}:${issue.entryId}:${index}`} className={styles[issue.severity]}>
                <strong>{issue.area}{issue.entryId ? ` / ${issue.entryId}` : ''}</strong>
                <span>{issue.message}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function ItemEditor({
  draft,
  update,
  remove,
}: {
  draft: ContentData;
  update: (path: string[], value: unknown) => void;
  remove: (path: string[]) => void;
}) {
  const equipment = asRecord(draft.equipment);
  const useEffect = asRecord(draft.use_effect);
  const effectType = String(useEffect.type || 'none');
  return (
    <>
      <Section title="Identity">
        <Field label="Display name" value={String(draft.display_name || '')} onChange={(value) => update(['display_name'], value)} />
        <Field label="Sprite key" value={String(draft.sprite || '')} onChange={(value) => update(['sprite'], value)} />
        <SelectField label="Category" value={String(draft.category || 'material')} options={['material', 'consumable', 'equipment', 'quest']} onChange={(value) => update(['category'], value)} />
        <Field label="Max stack" type="number" min={1} value={asNumber(draft.max_stack, 99)} onChange={(value) => update(['max_stack'], Number(value))} />
        <Field label="Base price" type="number" min={0} value={asNumber(draft.base_price, 1)} onChange={(value) => update(['base_price'], Number(value))} />
        <CheckboxField label="Sellable" checked={asBoolean(draft.sellable, true)} onChange={(value) => update(['sellable'], value)} />
        <label className={`${styles.field} ${styles.fullField}`}>
          <span>Description</span>
          <textarea value={String(draft.description || '')} onChange={(event) => update(['description'], event.target.value)} />
        </label>
      </Section>

      <Section title="Use Effect">
        <SelectField
          label="Effect type"
          value={effectType}
          options={['none', 'heal', 'restore_mana', 'restore_prayer', 'buff', 'teleport', 'learn_spell', 'dig', 'open_crate']}
          onChange={(value) => {
            if (value === 'none') remove(['use_effect']);
            else update(['use_effect'], defaultUseEffect(value));
          }}
        />
        {['heal', 'restore_mana', 'restore_prayer', 'buff'].includes(effectType) && (
          <Field label="Amount / power" type="number" value={asNumber(useEffect.amount, 1)} onChange={(value) => update(['use_effect', 'amount'], Number(value))} />
        )}
        {effectType === 'buff' && (
          <>
            <Field label="Stat" value={String(useEffect.stat || 'attack')} onChange={(value) => update(['use_effect', 'stat'], value)} />
            <Field label="Duration (ms)" type="number" value={asNumber(useEffect.duration_ms, 60000)} onChange={(value) => update(['use_effect', 'duration_ms'], Number(value))} />
          </>
        )}
        {effectType === 'learn_spell' && (
          <Field label="Spell ID" value={String(useEffect.spell_id || '')} onChange={(value) => update(['use_effect', 'spell_id'], value)} />
        )}
        {effectType === 'teleport' && (
          <>
            <Field label="Destination" value={String(useEffect.destination || 'overworld')} onChange={(value) => update(['use_effect', 'destination'], value)} />
            <Field label="Target X" type="number" value={asNumber(useEffect.x)} onChange={(value) => update(['use_effect', 'x'], Number(value))} />
            <Field label="Target Y" type="number" value={asNumber(useEffect.y)} onChange={(value) => update(['use_effect', 'y'], Number(value))} />
          </>
        )}
        {effectType === 'open_crate' && (
          <>
            <Field label="Tier" value={String(useEffect.tier || 'artisan')} onChange={(value) => update(['use_effect', 'tier'], value)} />
            <Field label="Bracket" value={String(useEffect.bracket || 'low')} onChange={(value) => update(['use_effect', 'bracket'], value)} />
          </>
        )}
      </Section>

      <Section title="Equipment">
        <CheckboxField
          label="Equippable"
          checked={Object.keys(equipment).length > 0}
          onChange={(checked) => checked
            ? update(['equipment'], { slot_type: 'weapon', attack_bonus: 0, strength_bonus: 0, defence_bonus: 0 })
            : remove(['equipment'])}
        />
        {Object.keys(equipment).length > 0 && (
          <>
            <SelectField label="Slot" value={String(equipment.slot_type || 'weapon')} options={['head', 'body', 'weapon', 'back', 'feet', 'ring', 'gloves', 'necklace', 'belt']} onChange={(value) => update(['equipment', 'slot_type'], value)} />
            <SelectField label="Weapon type" value={String(equipment.weapon_type || 'melee')} options={['melee', 'ranged']} onChange={(value) => update(['equipment', 'weapon_type'], value)} />
            <Field label="Attack requirement" type="number" min={0} value={asNumber(equipment.attack_level_required)} onChange={(value) => update(['equipment', 'attack_level_required'], Number(value))} />
            <Field label="Defence requirement" type="number" min={0} value={asNumber(equipment.defence_level_required)} onChange={(value) => update(['equipment', 'defence_level_required'], Number(value))} />
            <Field label="Ranged requirement" type="number" min={0} value={asNumber(equipment.ranged_level_required)} onChange={(value) => update(['equipment', 'ranged_level_required'], Number(value))} />
            <Field label="Magic requirement" type="number" min={0} value={asNumber(equipment.magic_level_required)} onChange={(value) => update(['equipment', 'magic_level_required'], Number(value))} />
            <Field label="Attack bonus" type="number" value={asNumber(equipment.attack_bonus)} onChange={(value) => update(['equipment', 'attack_bonus'], Number(value))} />
            <Field label="Strength bonus" type="number" value={asNumber(equipment.strength_bonus)} onChange={(value) => update(['equipment', 'strength_bonus'], Number(value))} />
            <Field label="Defence bonus" type="number" value={asNumber(equipment.defence_bonus)} onChange={(value) => update(['equipment', 'defence_bonus'], Number(value))} />
            <Field label="Magic bonus" type="number" value={asNumber(equipment.magic_bonus)} onChange={(value) => update(['equipment', 'magic_bonus'], Number(value))} />
            <Field label="Ranged strength" type="number" value={asNumber(equipment.ranged_strength_bonus)} onChange={(value) => update(['equipment', 'ranged_strength_bonus'], Number(value))} />
            <Field label="Range (tiles)" type="number" min={1} value={asNumber(equipment.range, 1)} onChange={(value) => update(['equipment', 'range'], Number(value))} />
          </>
        )}
      </Section>
    </>
  );
}

function EnemyEditor({
  draft,
  update,
}: {
  draft: ContentData;
  update: (path: string[], value: unknown) => void;
}) {
  const stats = asRecord(draft.stats);
  const rewards = asRecord(draft.rewards);
  const behaviors = asRecord(draft.behaviors);
  const loot = Array.isArray(draft.loot) ? draft.loot.map(asRecord) : [];
  const statFields: Array<[string, string, number]> = [
    ['level', 'Combat level', 1],
    ['max_hp', 'Max HP', 10],
    ['damage', 'Max hit', 1],
    ['attack_bonus', 'Attack bonus', 0],
    ['defence_bonus', 'Defence bonus', 0],
    ['attack_range', 'Attack range', 1],
    ['aggro_range', 'Aggro range', 4],
    ['chase_range', 'Chase range', 6],
    ['move_cooldown_ms', 'Move cooldown (ms)', 600],
    ['attack_cooldown_ms', 'Attack cooldown (ms)', 2000],
    ['respawn_time_ms', 'Respawn time (ms)', 10000],
  ];
  return (
    <>
      <Section title="Identity">
        <Field label="Display name" value={String(draft.display_name || '')} onChange={(value) => update(['display_name'], value)} />
        <Field label="Sprite key" value={String(draft.sprite || '')} onChange={(value) => update(['sprite'], value)} />
        <SelectField label="Animation" value={String(draft.animation_type || 'standard')} options={['standard', 'blob', 'humanoid', 'quadruped', 'flying']} onChange={(value) => update(['animation_type'], value)} />
        <Field label="Size (tiles)" type="number" min={1} value={asNumber(draft.size, 1)} onChange={(value) => update(['size'], Number(value))} />
        <Field label="Tags" value={(Array.isArray(draft.tags) ? draft.tags : []).join(', ')} onChange={(value) => update(['tags'], value.split(',').map((tag) => tag.trim()).filter(Boolean))} help="Comma-separated, used by equipment type bonuses." />
        <label className={`${styles.field} ${styles.fullField}`}>
          <span>Description</span>
          <textarea value={String(draft.description || '')} onChange={(event) => update(['description'], event.target.value)} />
        </label>
      </Section>

      <Section title="Combat Stats">
        {statFields.map(([key, label, fallback]) => (
          <Field key={key} label={label} type="number" value={asNumber(stats[key], fallback)} onChange={(value) => update(['stats', key], Number(value))} />
        ))}
      </Section>

      <Section title="Rewards">
        <Field label="XP per kill" type="number" min={0} value={asNumber(rewards.exp_base, 10)} onChange={(value) => update(['rewards', 'exp_base'], Number(value))} />
        <Field label="Gold minimum" type="number" min={0} value={asNumber(rewards.gold_min, 1)} onChange={(value) => update(['rewards', 'gold_min'], Number(value))} />
        <Field label="Gold maximum" type="number" min={0} value={asNumber(rewards.gold_max, 5)} onChange={(value) => update(['rewards', 'gold_max'], Number(value))} />
      </Section>

      <Section title="Behavior">
        <CheckboxField label="Hostile" checked={asBoolean(behaviors.hostile)} onChange={(value) => update(['behaviors', 'hostile'], value)} />
        <CheckboxField label="Friendly" checked={asBoolean(behaviors.friendly)} onChange={(value) => update(['behaviors', 'friendly'], value)} />
        <CheckboxField label="Wanders" checked={asBoolean(behaviors.wander_enabled)} onChange={(value) => update(['behaviors', 'wander_enabled'], value)} />
        <CheckboxField label="No shadow" checked={asBoolean(behaviors.no_shadow)} onChange={(value) => update(['behaviors', 'no_shadow'], value)} />
        <Field label="Wander radius" type="number" min={0} value={asNumber(behaviors.wander_radius, 3)} onChange={(value) => update(['behaviors', 'wander_radius'], Number(value))} />
        <Field label="Pause minimum (ms)" type="number" min={0} value={asNumber(behaviors.wander_pause_min_ms, 2000)} onChange={(value) => update(['behaviors', 'wander_pause_min_ms'], Number(value))} />
        <Field label="Pause maximum (ms)" type="number" min={0} value={asNumber(behaviors.wander_pause_max_ms, 5000)} onChange={(value) => update(['behaviors', 'wander_pause_max_ms'], Number(value))} />
      </Section>

      <section className={styles.formSection}>
        <div className={styles.sectionHeader}>
          <h3>Direct Loot</h3>
          <button onClick={() => update(['loot'], [...loot, { item_id: '', drop_chance: 0.1, quantity_min: 1, quantity_max: 1 }])}>
            <Plus size={14} /> Add drop
          </button>
        </div>
        <div className={styles.lootTable}>
          {loot.map((drop, index) => (
            <div className={styles.lootRow} key={`${String(drop.item_id)}:${index}`}>
              <Field label="Item ID" value={String(drop.item_id || '')} onChange={(value) => {
                const next = [...loot];
                next[index] = { ...drop, item_id: value };
                update(['loot'], next);
              }} />
              <Field label="Chance" type="number" min={0} step={0.001} value={asNumber(drop.drop_chance, 0.1)} onChange={(value) => {
                const next = [...loot];
                next[index] = { ...drop, drop_chance: Number(value) };
                update(['loot'], next);
              }} />
              <Field label="Min qty" type="number" min={1} value={asNumber(drop.quantity_min, 1)} onChange={(value) => {
                const next = [...loot];
                next[index] = { ...drop, quantity_min: Number(value) };
                update(['loot'], next);
              }} />
              <Field label="Max qty" type="number" min={1} value={asNumber(drop.quantity_max, 1)} onChange={(value) => {
                const next = [...loot];
                next[index] = { ...drop, quantity_max: Number(value) };
                update(['loot'], next);
              }} />
              <button className={styles.iconDelete} onClick={() => update(['loot'], loot.filter((_, dropIndex) => dropIndex !== index))}>
                <Trash2 size={15} />
              </button>
            </div>
          ))}
          {loot.length === 0 && <p className={styles.inlineEmpty}>No direct drops. Loot tables already in the TOML are preserved.</p>}
        </div>
      </section>
    </>
  );
}

function AttackEditor({
  draft,
  update,
}: {
  draft: ContentData;
  update: (path: string[], value: unknown) => void;
}) {
  return (
    <>
      <Section title="Attack Definition">
        <Field label="Name" value={String(draft.name || '')} onChange={(value) => update(['name'], value)} />
        <SelectField label="Type" value={String(draft.spell_type || 'damage')} options={['damage', 'heal', 'utility']} onChange={(value) => update(['spell_type'], value)} />
        <Field label="Mana cost" type="number" min={0} value={asNumber(draft.mana_cost)} onChange={(value) => update(['mana_cost'], Number(value))} />
        <Field label="Cooldown (ms)" type="number" min={1} value={asNumber(draft.cooldown_ms, 1000)} onChange={(value) => update(['cooldown_ms'], Number(value))} />
        <Field label="Base power" type="number" min={0} value={asNumber(draft.base_power)} onChange={(value) => update(['base_power'], Number(value))} />
        <Field label="Effect sprite" value={String(draft.effect_sprite || '')} onChange={(value) => update(['effect_sprite'], value)} />
        <Field label="Pushback distance" type="number" min={0} value={asNumber(draft.pushback_distance)} onChange={(value) => update(['pushback_distance'], Number(value))} />
        <Field label="Wall damage / tile" type="number" min={0} value={asNumber(draft.wall_slam_damage_per_tile)} onChange={(value) => update(['wall_slam_damage_per_tile'], Number(value))} />
        <label className={`${styles.field} ${styles.fullField}`}>
          <span>Description</span>
          <textarea value={String(draft.description || '')} onChange={(event) => update(['description'], event.target.value)} />
        </label>
      </Section>
      <section className={styles.formSection}>
        <h3>Quick Read</h3>
        <div className={styles.attackPreview}>
          <Swords size={22} />
          <strong>{asNumber(draft.base_power)} power</strong>
          <span>{(asNumber(draft.cooldown_ms, 1000) / 1000).toFixed(1)}s cooldown</span>
          <span>{asNumber(draft.mana_cost)} mana</span>
          <span>{(asNumber(draft.base_power) / Math.max(0.1, asNumber(draft.cooldown_ms, 1000) / 1000)).toFixed(1)} base power/sec</span>
        </div>
      </section>
    </>
  );
}

function BalanceLab({ entries }: { entries: ContentEntry[] }) {
  const [profile, setProfile] = useState<PlayerBalanceProfile>({
    attackLevel: 20,
    strengthLevel: 20,
    defenceLevel: 20,
    attackBonus: 8,
    strengthBonus: 6,
    defenceBonus: 8,
    attackCooldownMs: 1000,
  });
  const enemies = entries.filter((entry) => entry.kind === 'enemy');
  const rows = calculateEnemyBalance(enemies, profile);
  const equipment = entries.filter(
    (entry) => entry.kind === 'item' && Object.keys(asRecord(entry.data.equipment)).length > 0
  ).sort((a, b) => equipmentPower(a.data) - equipmentPower(b.data));

  const setProfileValue = (key: keyof PlayerBalanceProfile, value: string) => {
    setProfile((current) => ({ ...current, [key]: Number(value) }));
  };

  return (
    <div className={styles.balancePage}>
      <section className={styles.profilePanel}>
        <div className={styles.panelTitle}>
          <div><Swords size={18} /><h2>Player Test Profile</h2></div>
          <span>Max hit: {playerMaxHit(profile)}</span>
        </div>
        <div className={styles.fieldGrid}>
          <Field label="Attack level" type="number" min={1} value={profile.attackLevel} onChange={(value) => setProfileValue('attackLevel', value)} />
          <Field label="Strength level" type="number" min={1} value={profile.strengthLevel} onChange={(value) => setProfileValue('strengthLevel', value)} />
          <Field label="Defence level" type="number" min={1} value={profile.defenceLevel} onChange={(value) => setProfileValue('defenceLevel', value)} />
          <Field label="Attack bonus" type="number" value={profile.attackBonus} onChange={(value) => setProfileValue('attackBonus', value)} />
          <Field label="Strength bonus" type="number" value={profile.strengthBonus} onChange={(value) => setProfileValue('strengthBonus', value)} />
          <Field label="Defence bonus" type="number" value={profile.defenceBonus} onChange={(value) => setProfileValue('defenceBonus', value)} />
          <Field label="Attack cooldown (ms)" type="number" min={100} value={profile.attackCooldownMs} onChange={(value) => setProfileValue('attackCooldownMs', value)} />
        </div>
      </section>

      <section className={styles.tablePanel}>
        <div className={styles.panelTitle}>
          <div><Shield size={18} /><h2>Enemy Progression</h2></div>
          <span>Uses the server hit and max-hit formulas</span>
        </div>
        <div className={styles.tableScroll}>
          <table>
            <thead>
              <tr><th>Enemy</th><th>Lvl</th><th>HP</th><th>Max</th><th>Hit %</th><th>TTK</th><th>Incoming DPS</th><th>XP/min</th><th>Gold/min</th></tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id}>
                  <td><strong>{row.name}</strong><small>{row.id}</small></td>
                  <td>{row.level}</td>
                  <td>{row.hp}</td>
                  <td>{row.maxHit}</td>
                  <td>{Math.round(row.playerHitChance * 100)}%</td>
                  <td className={row.timeToKill > 90 ? styles.outlier : ''}>{row.timeToKill.toFixed(1)}s</td>
                  <td>{row.enemyDps.toFixed(2)}</td>
                  <td>{Math.round(row.expPerMinute)}</td>
                  <td>{Math.round(row.goldPerMinute)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className={styles.tablePanel}>
        <div className={styles.panelTitle}>
          <div><Package size={18} /><h2>Equipment Power Curve</h2></div>
          <span>Attack + 2x strength + defence + magic/ranged weighting</span>
        </div>
        <div className={styles.equipmentGrid}>
          {equipment.map((entry) => {
            const equipmentData = asRecord(entry.data.equipment);
            const requirement = Math.max(
              asNumber(equipmentData.attack_level_required),
              asNumber(equipmentData.defence_level_required),
              asNumber(equipmentData.ranged_level_required),
              asNumber(equipmentData.magic_level_required)
            );
            return (
              <div key={`${entry.file}:${entry.id}`} className={styles.equipmentCard}>
                <strong>{String(entry.data.display_name || entry.id)}</strong>
                <span>Power {equipmentPower(entry.data).toFixed(1)}</span>
                <small>Req {requirement} / {String(equipmentData.slot_type || 'none')}</small>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function MapTools({
  entries,
  issues,
  chunks,
  currentWorld,
  setChunks,
  setWorldBounds,
  onOpenMap,
  setNotice,
}: {
  entries: ContentEntry[];
  issues: ReturnType<typeof validateContent>;
  chunks: Map<string, Chunk>;
  currentWorld: string;
  setChunks: (chunks: Map<string, Chunk>) => void;
  setWorldBounds: (bounds: { minCx: number; maxCx: number; minCy: number; maxCy: number }) => void;
  onOpenMap: () => void;
  setNotice: (message: string) => void;
}) {
  const [minCx, setMinCx] = useState(0);
  const [maxCx, setMaxCx] = useState(0);
  const [minCy, setMinCy] = useState(0);
  const [maxCy, setMaxCy] = useState(0);
  const [primaryTile, setPrimaryTile] = useState(1);
  const [secondaryTile, setSecondaryTile] = useState(1);
  const [pattern, setPattern] = useState('solid');
  const [borderCollision, setBorderCollision] = useState(false);
  const [overwrite, setOverwrite] = useState(false);
  const mapIssues = issues.filter((issue) => issue.area === 'Maps');
  const knownEntities = entries.filter((entry) => entry.kind === 'enemy' || entry.kind === 'npc').length;

  const generate = () => {
    const lowX = Math.min(minCx, maxCx);
    const highX = Math.max(minCx, maxCx);
    const lowY = Math.min(minCy, maxCy);
    const highY = Math.max(minCy, maxCy);
    const chunkCount = (highX - lowX + 1) * (highY - lowY + 1);
    if (chunkCount > 400 && !window.confirm(`Generate ${chunkCount} chunks?`)) return;

    const next = new Map(chunks);
    let changed = 0;
    for (let cy = lowY; cy <= highY; cy++) {
      for (let cx = lowX; cx <= highX; cx++) {
        const key = chunkKey({ cx, cy });
        const existing = next.get(key);
        if (existing && !overwrite) continue;
        const base = existing || chunkManager.createEmptyChunk({ cx, cy });
        const ground = new Array(base.width * base.height).fill(primaryTile);
        const collision = new Uint8Array(base.collision);
        for (let y = 0; y < base.height; y++) {
          for (let x = 0; x < base.width; x++) {
            const index = y * base.width + x;
            if (pattern === 'checker' && (x + y + cx + cy) % 2 !== 0) {
              ground[index] = secondaryTile;
            } else if (pattern === 'noise') {
              const hash = Math.abs((cx * 32 + x) * 73856093 ^ (cy * 32 + y) * 19349663);
              if (hash % 100 < 28) ground[index] = secondaryTile;
            }
            if (borderCollision && (x === 0 || y === 0 || x === base.width - 1 || y === base.height - 1)) {
              collision[Math.floor(index / 8)] |= 1 << (index % 8);
            }
          }
        }
        next.set(key, { ...base, layers: { ...base.layers, ground }, collision, dirty: true });
        changed++;
      }
    }
    setChunks(next);
    const coords = Array.from(next.values()).map((chunk) => chunk.coord);
    setWorldBounds({
      minCx: Math.min(...coords.map((coord) => coord.cx)),
      maxCx: Math.max(...coords.map((coord) => coord.cx)),
      minCy: Math.min(...coords.map((coord) => coord.cy)),
      maxCy: Math.max(...coords.map((coord) => coord.cy)),
    });
    setNotice(`Generated or updated ${changed} chunks`);
  };

  return (
    <div className={styles.mapToolsPage}>
      <section className={styles.generatorPanel}>
        <div className={styles.panelTitle}>
          <div><MapIcon size={18} /><h2>Region Generator</h2></div>
          <span>{currentWorld}</span>
        </div>
        <p>Create chunk-sized terrain blocks as a starting point, then refine them in the map editor.</p>
        <div className={styles.fieldGrid}>
          <Field label="Min chunk X" type="number" value={minCx} onChange={(value) => setMinCx(Number(value))} />
          <Field label="Max chunk X" type="number" value={maxCx} onChange={(value) => setMaxCx(Number(value))} />
          <Field label="Min chunk Y" type="number" value={minCy} onChange={(value) => setMinCy(Number(value))} />
          <Field label="Max chunk Y" type="number" value={maxCy} onChange={(value) => setMaxCy(Number(value))} />
          <Field label="Primary tile ID" type="number" min={0} value={primaryTile} onChange={(value) => setPrimaryTile(Number(value))} />
          <Field label="Secondary tile ID" type="number" min={0} value={secondaryTile} onChange={(value) => setSecondaryTile(Number(value))} />
          <SelectField label="Pattern" value={pattern} options={['solid', 'checker', 'noise']} onChange={setPattern} />
          <CheckboxField label="Block chunk borders" checked={borderCollision} onChange={setBorderCollision} />
          <CheckboxField label="Overwrite existing chunks" checked={overwrite} onChange={setOverwrite} />
        </div>
        <div className={styles.generatorActions}>
          <button className={styles.primaryButton} onClick={generate}><Plus size={15} /> Generate Region</button>
          <button onClick={onOpenMap}>Open Map Editor</button>
        </div>
      </section>

      <section className={styles.issuePanel}>
        <div className={styles.panelTitle}>
          <div><AlertTriangle size={18} /><h2>Map Audit</h2></div>
          <span>{chunks.size} chunks / {knownEntities} known entities</span>
        </div>
        {mapIssues.length === 0 ? (
          <div className={styles.successState}>No broken entity references, duplicate unique IDs, or empty chunks found.</div>
        ) : (
          <div className={styles.issueList}>
            {mapIssues.map((issue, index) => (
              <div key={`${issue.entryId}:${index}`} className={styles[issue.severity]}>
                <strong>{issue.entryId || 'Map'}</strong>
                <span>{issue.message}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
