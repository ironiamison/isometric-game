import type {
  ContentEntry,
  EnemyBalanceRow,
  PlayerBalanceProfile,
} from './types';

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function number(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function hitChance(
  attackLevel: number,
  attackBonus: number,
  defenceLevel: number,
  defenceBonus: number
): number {
  const attackMax = Math.max(1, (attackLevel + 20) * (attackBonus + 20));
  const defenceMax = Math.max(1, (defenceLevel + 20) * (defenceBonus + 20));
  return attackMax <= defenceMax
    ? attackMax / (2 * defenceMax)
    : 1 - defenceMax / (2 * attackMax);
}

export function playerMaxHit(profile: PlayerBalanceProfile): number {
  return Math.max(1, Math.floor(
    1 + profile.strengthLevel / 16 + profile.strengthBonus / 4
  ));
}

export function calculateEnemyBalance(
  enemies: ContentEntry[],
  profile: PlayerBalanceProfile
): EnemyBalanceRow[] {
  const maxHit = playerMaxHit(profile);
  const averagePlayerDamage = (1 + maxHit) / 2;
  const byId = new Map(enemies.map((entry) => [entry.id, entry]));

  const resolveData = (
    entry: ContentEntry,
    resolving = new Set<string>()
  ): Record<string, unknown> => {
    const parentId = typeof entry.data.extends === 'string' ? entry.data.extends : '';
    if (!parentId || resolving.has(entry.id)) return entry.data;
    const parent = byId.get(parentId);
    if (!parent) return entry.data;
    const nextResolving = new Set(resolving);
    nextResolving.add(entry.id);
    const parentData = resolveData(parent, nextResolving);
    return {
      ...parentData,
      ...entry.data,
      stats: { ...record(parentData.stats), ...record(entry.data.stats) },
      rewards: { ...record(parentData.rewards), ...record(entry.data.rewards) },
      behaviors: { ...record(parentData.behaviors), ...record(entry.data.behaviors) },
    };
  };

  return enemies.flatMap((entry) => {
    const data = resolveData(entry);
    const stats = record(data.stats);
    const rewards = record(data.rewards);
    const behaviors = record(data.behaviors);
    if (behaviors.hostile !== true && number(stats.damage, 0) <= 0) return [];
    const level = number(stats.level, 1);
    const hp = Math.max(1, number(stats.max_hp, 100));
    const enemyMaxHit = Math.max(0, number(stats.damage, 0));
    const enemyCooldown = Math.max(100, number(stats.attack_cooldown_ms, 800));
    const enemyAccuracy = hitChance(
      level,
      number(stats.attack_bonus),
      profile.defenceLevel,
      profile.defenceBonus
    );
    const enemyAverageDamage = enemyMaxHit > 0 ? (1 + enemyMaxHit) / 2 : 0;
    const enemyDps = enemyAccuracy * enemyAverageDamage / (enemyCooldown / 1000);

    const playerAccuracy = hitChance(
      profile.attackLevel,
      profile.attackBonus,
      level,
      number(stats.defence_bonus)
    );
    const playerDps = playerAccuracy * averagePlayerDamage
      / (Math.max(100, profile.attackCooldownMs) / 1000);
    const timeToKill = hp / Math.max(0.01, playerDps);
    const killsPerMinute = 60 / Math.max(0.1, timeToKill);
    const averageGold = (
      number(rewards.gold_min, 1) + number(rewards.gold_max, 5)
    ) / 2;

    return [{
      id: entry.id,
      name: String(data.display_name || entry.id),
      level,
      hp,
      maxHit: enemyMaxHit,
      enemyDps,
      playerHitChance: playerAccuracy,
      playerDps,
      timeToKill,
      expPerMinute: number(rewards.exp_base, 10) * killsPerMinute,
      goldPerMinute: averageGold * killsPerMinute,
    }];
  }).sort((a, b) => a.level - b.level || a.name.localeCompare(b.name));
}

export function equipmentPower(data: Record<string, unknown>): number {
  const equipment = record(data.equipment);
  return number(equipment.attack_bonus)
    + number(equipment.strength_bonus) * 2
    + number(equipment.defence_bonus)
    + number(equipment.magic_bonus) * 1.5
    + number(equipment.ranged_strength_bonus) * 2;
}
