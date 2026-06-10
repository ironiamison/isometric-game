export type ContentKind = 'item' | 'enemy' | 'npc' | 'attack';

export type ContentData = Record<string, unknown>;

export interface ContentFile {
  path: string;
  kind: ContentKind;
  entries: Record<string, ContentData>;
  error?: string;
}

export interface ContentCatalog {
  files: ContentFile[];
}

export interface ContentEntry {
  id: string;
  kind: ContentKind;
  file: string;
  data: ContentData;
}

export type IssueSeverity = 'error' | 'warning';

export interface ContentIssue {
  severity: IssueSeverity;
  area: string;
  entryId?: string;
  message: string;
}

export interface PlayerBalanceProfile {
  attackLevel: number;
  strengthLevel: number;
  defenceLevel: number;
  attackBonus: number;
  strengthBonus: number;
  defenceBonus: number;
  attackCooldownMs: number;
}

export interface EnemyBalanceRow {
  id: string;
  name: string;
  level: number;
  hp: number;
  maxHit: number;
  enemyDps: number;
  playerHitChance: number;
  playerDps: number;
  timeToKill: number;
  expPerMinute: number;
  goldPerMinute: number;
}
