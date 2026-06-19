const BASE = '/api/stats';
const AUTH_BASE = '/api';
const FETCH_TIMEOUT_MS = 8_000;

/** Poll interval aligned with server leaderboard cache TTL (~10s). */
export const LIVE_STATS_POLL_MS = 10_000;

async function get<T>(path: string): Promise<T> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const r = await fetch(`${BASE}${path}`, { signal: controller.signal });
    if (!r.ok) throw new Error(`API error: ${r.status}`);
    return r.json();
  } finally {
    clearTimeout(timeout);
  }
}

async function authGet<T>(path: string, token: string): Promise<T> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const r = await fetch(`${AUTH_BASE}${path}`, {
      signal: controller.signal,
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!r.ok) throw new Error(`API error: ${r.status}`);
    return r.json();
  } finally {
    clearTimeout(timeout);
  }
}

export interface Overview {
  online_players: number;
  total_characters: number;
  total_accounts: number;
}

export interface OnlinePlayer {
  name: string;
  combat_level: number;
  hitpoints_level: number;
  attack_level: number;
  strength_level: number;
  defence_level: number;
  ranged_level: number;
  total_level: number;
}

export interface LeaderboardEntry {
  name: string;
  combat_level: number;
  hitpoints_level: number;
  attack_level: number;
  strength_level: number;
  defence_level: number;
  ranged_level: number;
  fishing_level: number;
  farming_level: number;
  smithing_level: number;
  prayer_level: number;
  magic_level: number;
  woodcutting_level: number;
  mining_level: number;
  alchemy_level: number;
  slayer_level: number;
  survivalist_level: number;
  total_level: number;
  played_time: number;
  monster_kills: number;
}

export type LeaderboardSort =
  | 'combat_level'
  | 'hitpoints_level'
  | 'attack_level'
  | 'strength_level'
  | 'defence_level'
  | 'ranged_level'
  | 'fishing_level'
  | 'farming_level'
  | 'smithing_level'
  | 'prayer_level'
  | 'magic_level'
  | 'woodcutting_level'
  | 'mining_level'
  | 'alchemy_level'
  | 'slayer_level'
  | 'survivalist_level'
  | 'total_level'
  | 'played_time'
  | 'monster_kills';

export interface PlayerProfileRanks {
  total_level: number;
  combat_level: number;
  hitpoints_level: number;
  attack_level: number;
  strength_level: number;
  defence_level: number;
  ranged_level: number;
  fishing_level: number;
  farming_level: number;
  smithing_level: number;
  prayer_level: number;
  magic_level: number;
  woodcutting_level: number;
  mining_level: number;
  alchemy_level: number;
  slayer_level: number;
  survivalist_level: number;
  monster_kills: number;
  played_time: number;
}

export interface PlayerProfileResponse {
  player: LeaderboardEntry;
  ranks: PlayerProfileRanks;
  total_characters: number;
}

export interface Equipment {
  slot_type: string;
  attack_level_required: number;
  defence_level_required: number;
  ranged_level_required: number;
  woodcutting_level_required: number;
  mining_level_required: number;
  magic_level_required: number;
  attack_bonus: number;
  strength_bonus: number;
  defence_bonus: number;
  ranged_strength_bonus: number;
  magic_bonus: number;
  weapon_type: string;
  range: number;
}

export interface Item {
  id: string;
  display_name: string;
  sprite: string;
  description: string;
  category: string;
  max_stack: number;
  base_price: number;
  sellable: boolean;
  equipment: Equipment | null;
}

export interface EntityLoot {
  item_id: string;
  drop_chance: number;
  quantity_min: number;
  quantity_max: number;
}

export interface LootTableEntry {
  item_id: string;
  weight: number;
  quantity_min: number;
  quantity_max: number;
}

export interface LootTable {
  name: string;
  chance: number;
  entries: LootTableEntry[];
}

export interface Entity {
  id: string;
  display_name: string;
  sprite: string;
  description: string;
  level: number;
  max_hp: number;
  damage: number;
  attack_bonus: number;
  defence_bonus: number;
  attack_range: number;
  aggro_range: number;
  respawn_time_ms: number;
  hostile: boolean;
  exp_base: number;
  gold_min: number;
  gold_max: number;
  loot: EntityLoot[];
  loot_tables: LootTable[];
  quest_ids: string[];
}

export interface CharacterInfo {
  id: number;
  name: string;
  level: number;
  playedTime: number;
}

export interface CharacterListResponse {
  success: boolean;
  characters?: CharacterInfo[];
  error?: string;
}

export const api = {
  overview: () => get<Overview>('/overview'),
  online: () => get<OnlinePlayer[]>('/online'),
  leaderboard: (sort: LeaderboardSort = 'total_level', limit = 50) =>
    get<LeaderboardEntry[]>(`/leaderboard?sort=${sort}&limit=${limit}`),
  playerProfile: (name: string) =>
    get<PlayerProfileResponse>(`/player/${encodeURIComponent(name)}`),
  characters: async (token: string) => {
    const resp = await authGet<CharacterListResponse>('/characters', token);
    if (!resp.success) throw new Error(resp.error ?? 'Not authenticated');
    return resp.characters ?? [];
  },
  items: () => get<Item[]>('/items'),
  entities: () => get<Entity[]>('/entities'),
};
