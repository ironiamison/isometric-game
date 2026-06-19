import type { LeaderboardEntry, LeaderboardSort } from '$lib/api';

export type LeaderboardCategory = {
  id: string;
  label: string;
  icon: string;
  sort: LeaderboardSort;
  title: string;
  subtitle: string;
  levelField: keyof LeaderboardEntry;
  valueLabel: string;
};

export const LEADERBOARD_CATEGORIES: LeaderboardCategory[] = [
  {
    id: 'overall',
    label: 'Overall',
    icon: '👑',
    sort: 'total_level',
    title: 'Overall Leaderboard',
    subtitle: 'The most accomplished adventurers in all of Solstead.',
    levelField: 'total_level',
    valueLabel: 'Total XP',
  },
  {
    id: 'skills',
    label: 'Skills',
    icon: '✦',
    sort: 'total_level',
    title: 'Skills Leaderboard',
    subtitle: 'Masters of every craft and trade across the realm.',
    levelField: 'total_level',
    valueLabel: 'Skill Points',
  },
  {
    id: 'combat',
    label: 'Combat',
    icon: '⚔',
    sort: 'combat_level',
    title: 'Combat Leaderboard',
    subtitle: 'The fiercest fighters in the overworld and dungeons.',
    levelField: 'combat_level',
    valueLabel: 'Combat XP',
  },
  {
    id: 'wealth',
    label: 'Wealth',
    icon: '🪙',
    sort: 'monster_kills',
    title: 'Wealth Leaderboard',
    subtitle: 'Fortune earned through trade, loot, and conquest.',
    levelField: 'total_level',
    valueLabel: 'Est. Wealth',
  },
  {
    id: 'clans',
    label: 'Clans',
    icon: '🛡',
    sort: 'total_level',
    title: 'Clan Leaderboard',
    subtitle: 'The strongest guilds shaping the world together.',
    levelField: 'total_level',
    valueLabel: 'Clan XP',
  },
  {
    id: 'exploration',
    label: 'Exploration',
    icon: '🧭',
    sort: 'played_time',
    title: 'Exploration Leaderboard',
    subtitle: 'Those who have wandered farthest across Solstead.',
    levelField: 'total_level',
    valueLabel: 'Time Explored',
  },
  {
    id: 'bosses',
    label: 'Bosses',
    icon: '💀',
    sort: 'monster_kills',
    title: 'Boss Hunters',
    subtitle: 'Slayers of the deadliest creatures in the world.',
    levelField: 'combat_level',
    valueLabel: 'Boss Kills',
  },
  {
    id: 'achievements',
    label: 'Achievements',
    icon: '🏆',
    sort: 'total_level',
    title: 'Achievement Hunters',
    subtitle: 'Players closest to completing every challenge.',
    levelField: 'total_level',
    valueLabel: 'Achievement Score',
  },
];

const CLAN_NAMES = [
  'Iron Vanguard',
  'Golden Forge',
  'Emerald Circle',
  'Storm Riders',
  'Shadow Pact',
  'Sunward Order',
  'Crimson Blades',
  'Silverwood Sentinels',
];

const CLAN_ICONS = ['🛡', '⚒', '🌿', '⚡', '🌙', '☀', '🗡', '🌲'];

export function clanForPlayer(name: string): { name: string; icon: string } {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash + name.charCodeAt(i) * (i + 1)) >>> 0;
  const idx = hash % CLAN_NAMES.length;
  return { name: CLAN_NAMES[idx], icon: CLAN_ICONS[idx] };
}

export function avatarHue(name: string): number {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
  return Math.abs(hash) % 360;
}

/** Display XP / score from available stats (no raw XP in API yet). */
export function scoreValue(category: LeaderboardCategory, entry: LeaderboardEntry): number {
  switch (category.id) {
    case 'wealth':
      return entry.monster_kills * 12_500 + entry.total_level * 8_000;
    case 'exploration':
      return entry.played_time;
    case 'bosses':
      return entry.monster_kills;
    case 'achievements':
      return entry.total_level * 42_000 + entry.monster_kills * 500;
    case 'combat':
      return entry.combat_level * 890_000 + entry.monster_kills * 1200;
    case 'skills':
      return sumSkills(entry) * 95_000;
    default:
      return entry.total_level * 2_987_654 + entry.monster_kills * 2500;
  }
}

export function formatScore(category: LeaderboardCategory, entry: LeaderboardEntry): string {
  if (category.id === 'exploration') {
    const hours = Math.floor(entry.played_time / 3600);
    const days = Math.floor(hours / 24);
    if (days > 0) return `${days}d ${hours % 24}h`;
    return `${hours}h`;
  }
  return scoreValue(category, entry).toLocaleString();
}

export function displayLevel(category: LeaderboardCategory, entry: LeaderboardEntry): number {
  const v = entry[category.levelField];
  return typeof v === 'number' ? v : entry.total_level;
}

function sumSkills(entry: LeaderboardEntry): number {
  return (
    entry.combat_level +
    entry.hitpoints_level +
    entry.attack_level +
    entry.strength_level +
    entry.defence_level +
    entry.ranged_level +
    entry.fishing_level +
    entry.farming_level +
    entry.smithing_level +
    entry.prayer_level +
    entry.magic_level +
    entry.woodcutting_level +
    entry.mining_level +
    entry.alchemy_level +
    entry.slayer_level +
    entry.survivalist_level
  );
}

export const MOCK_TOP_CLANS = [
  { rank: 1, name: 'Iron Vanguard', icon: '🛡', xp: 512_438_910 },
  { rank: 2, name: 'Golden Forge', icon: '⚒', xp: 498_221_340 },
  { rank: 3, name: 'Emerald Circle', icon: '🌿', xp: 476_890_120 },
  { rank: 4, name: 'Storm Riders', icon: '⚡', xp: 445_102_880 },
  { rank: 5, name: 'Shadow Pact', icon: '🌙', xp: 421_556_700 },
];

export function seasonCountdown(): string {
  const end = new Date();
  end.setUTCMonth(end.getUTCMonth() + 1);
  end.setUTCDate(1);
  const ms = end.getTime() - Date.now();
  const days = Math.max(0, Math.floor(ms / 86_400_000));
  const hours = Math.max(0, Math.floor((ms % 86_400_000) / 3_600_000));
  const mins = Math.max(0, Math.floor((ms % 3_600_000) / 60_000));
  return `${days}d ${hours}h ${mins}m`;
}
