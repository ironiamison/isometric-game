const BASE = '/api/stats'

export interface Overview {
  online_players: number
  total_characters: number
  total_accounts: number
}

export interface OnlinePlayer {
  name: string
  combat_level: number
  hitpoints_level: number
  combat_skill_level: number
  total_level: number
}

export interface LeaderboardEntry {
  name: string
  combat_level: number
  hitpoints_level: number
  combat_skill_level: number
  fishing_level: number
  farming_level: number
  smithing_level: number
  prayer_level: number
  magic_level: number
  woodcutting_level: number
  mining_level: number
  alchemy_level: number
  slayer_level: number
  total_level: number
  played_time: number
  monster_kills: number
}

export type LeaderboardSort =
  | 'combat_level'
  | 'hitpoints_level'
  | 'combat_skill_level'
  | 'fishing_level'
  | 'farming_level'
  | 'smithing_level'
  | 'prayer_level'
  | 'magic_level'
  | 'woodcutting_level'
  | 'mining_level'
  | 'alchemy_level'
  | 'slayer_level'
  | 'total_level'
  | 'played_time'
  | 'monster_kills'

export interface PlayerProfileRanks {
  total_level: number
  combat_level: number
  hitpoints_level: number
  combat_skill_level: number
  fishing_level: number
  farming_level: number
  smithing_level: number
  prayer_level: number
  magic_level: number
  woodcutting_level: number
  mining_level: number
  alchemy_level: number
  slayer_level: number
  monster_kills: number
  played_time: number
}

export interface PlayerProfileResponse {
  player: LeaderboardEntry
  ranks: PlayerProfileRanks
  total_characters: number
}

export interface Equipment {
  slot_type: string
  attack_level_required: number
  defence_level_required: number
  attack_bonus: number
  strength_bonus: number
  defence_bonus: number
  weapon_type: string
  range: number
}

export interface Item {
  id: string
  display_name: string
  sprite: string
  description: string
  category: string
  max_stack: number
  base_price: number
  sellable: boolean
  equipment: Equipment | null
}

async function get<T>(path: string): Promise<T> {
  const r = await fetch(`${BASE}${path}`)
  if (!r.ok) throw new Error(`API error: ${r.status}`)
  return r.json()
}

export const api = {
  overview: () => get<Overview>('/overview'),
  online: () => get<OnlinePlayer[]>('/online'),
  leaderboard: (sort: LeaderboardSort = 'total_level', limit = 50) =>
    get<LeaderboardEntry[]>(`/leaderboard?sort=${sort}&limit=${limit}`),
  playerProfile: (name: string) =>
    get<PlayerProfileResponse>(`/player/${encodeURIComponent(name)}`),
  items: () => get<Item[]>('/items'),
}
