import type { LeaderboardEntry, LeaderboardSort } from '$lib/api';

export type NumericField = Exclude<keyof LeaderboardEntry, 'name'>;

export type Metric = {
  label: string;
  sort: LeaderboardSort;
  field: NumericField;
  hint: string;
};

export const METRIC_GROUPS: { title: string; metrics: Metric[] }[] = [
  {
    title: 'Showcase',
    metrics: [
      { label: 'Total Level', sort: 'total_level', field: 'total_level', hint: 'Combined levels' },
      { label: 'Combat Power', sort: 'combat_level', field: 'combat_level', hint: 'Melee + HP combined' },
      { label: 'Played Time', sort: 'played_time', field: 'played_time', hint: 'Time played' },
      { label: 'Monster Kills', sort: 'monster_kills', field: 'monster_kills', hint: 'Lifetime kills' },
    ],
  },
  {
    title: 'Combat',
    metrics: [
      { label: 'Attack', sort: 'attack_level', field: 'attack_level', hint: 'Melee accuracy' },
      { label: 'Strength', sort: 'strength_level', field: 'strength_level', hint: 'Melee damage' },
      { label: 'Defence', sort: 'defence_level', field: 'defence_level', hint: 'Evasion' },
      { label: 'Ranged', sort: 'ranged_level', field: 'ranged_level', hint: 'Ranged combat' },
      { label: 'Hitpoints', sort: 'hitpoints_level', field: 'hitpoints_level', hint: 'HP level' },
      { label: 'Prayer', sort: 'prayer_level', field: 'prayer_level', hint: 'Prayer level' },
      { label: 'Magic', sort: 'magic_level', field: 'magic_level', hint: 'Magic level' },
      { label: 'Slayer', sort: 'slayer_level', field: 'slayer_level', hint: 'Slayer level' },
    ],
  },
  {
    title: 'Gathering',
    metrics: [
      { label: 'Fishing', sort: 'fishing_level', field: 'fishing_level', hint: 'Fishing level' },
      { label: 'Farming', sort: 'farming_level', field: 'farming_level', hint: 'Farming level' },
      { label: 'Woodcutting', sort: 'woodcutting_level', field: 'woodcutting_level', hint: 'Woodcutting level' },
      { label: 'Mining', sort: 'mining_level', field: 'mining_level', hint: 'Mining level' },
      { label: 'Survivalist', sort: 'survivalist_level', field: 'survivalist_level', hint: 'Survivalist level' },
    ],
  },
  {
    title: 'Crafting',
    metrics: [
      { label: 'Smithing', sort: 'smithing_level', field: 'smithing_level', hint: 'Smithing level' },
      { label: 'Alchemy', sort: 'alchemy_level', field: 'alchemy_level', hint: 'Alchemy level' },
    ],
  },
];

export const METRICS = METRIC_GROUPS.flatMap((group) => group.metrics);

export function metricValue(metric: Metric, entry: LeaderboardEntry): string {
  if (metric.sort === 'played_time') {
    const days = Math.floor(entry.played_time / 86_400);
    const hours = Math.floor((entry.played_time % 86_400) / 3_600);
    if (days > 0) return `${days}d ${hours}h`;
    const minutes = Math.floor((entry.played_time % 3_600) / 60);
    return `${hours}h ${minutes}m`;
  }
  const raw = entry[metric.field];
  return Number(raw).toLocaleString();
}
