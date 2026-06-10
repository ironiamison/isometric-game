import type { Chunk } from '@/types';
import type { ContentEntry, ContentIssue } from './types';

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function number(value: unknown, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

export function validateContent(
  entries: ContentEntry[],
  chunks: Iterable<Chunk>
): ContentIssue[] {
  const issues: ContentIssue[] = [];
  const ids = new Map<string, ContentEntry[]>();
  const itemIds = new Set(entries.filter((entry) => entry.kind === 'item').map((entry) => entry.id));
  const entityIds = new Set(
    entries.filter((entry) => entry.kind === 'enemy' || entry.kind === 'npc')
      .map((entry) => entry.id)
  );

  for (const entry of entries) {
    const namespace = entry.kind === 'enemy' || entry.kind === 'npc' ? 'entity' : entry.kind;
    const duplicateKey = `${namespace}:${entry.id}`;
    const duplicates = ids.get(duplicateKey) || [];
    duplicates.push(entry);
    ids.set(duplicateKey, duplicates);

    if (entry.kind === 'item') {
      if (!entry.data.display_name) {
        issues.push({ severity: 'warning', area: 'Items', entryId: entry.id, message: 'Missing display_name.' });
      }
      if (!entry.data.sprite) {
        issues.push({ severity: 'error', area: 'Items', entryId: entry.id, message: 'Missing sprite.' });
      }
      if (number(entry.data.max_stack, 99) < 1) {
        issues.push({ severity: 'error', area: 'Items', entryId: entry.id, message: 'max_stack must be at least 1.' });
      }
      if (number(entry.data.base_price, 1) < 0) {
        issues.push({ severity: 'error', area: 'Items', entryId: entry.id, message: 'base_price cannot be negative.' });
      }
    }

    if (entry.kind === 'enemy') {
      const stats = record(entry.data.stats);
      const rewards = record(entry.data.rewards);
      const behaviors = record(entry.data.behaviors);
      const isCombatEnemy = behaviors.hostile === true || number(stats.damage, 0) > 0;
      if (!entry.data.sprite) {
        issues.push({ severity: 'error', area: 'Enemies', entryId: entry.id, message: 'Missing sprite.' });
      }
      if (isCombatEnemy && 'max_hp' in stats && number(stats.max_hp, 0) <= 0) {
        issues.push({ severity: 'error', area: 'Enemies', entryId: entry.id, message: 'max_hp must be greater than 0.' });
      }
      if (isCombatEnemy && 'attack_cooldown_ms' in stats && number(stats.attack_cooldown_ms, 0) <= 0) {
        issues.push({ severity: 'error', area: 'Enemies', entryId: entry.id, message: 'attack_cooldown_ms must be greater than 0.' });
      }
      if (isCombatEnemy && number(stats.chase_range, 0) < number(stats.aggro_range, 0)) {
        issues.push({ severity: 'warning', area: 'Enemies', entryId: entry.id, message: 'chase_range is smaller than aggro_range.' });
      }
      if (number(rewards.gold_max, 0) < number(rewards.gold_min, 0)) {
        issues.push({ severity: 'error', area: 'Enemies', entryId: entry.id, message: 'gold_max is smaller than gold_min.' });
      }

      const loot = Array.isArray(entry.data.loot) ? entry.data.loot : [];
      for (const rawLoot of loot) {
        const lootEntry = record(rawLoot);
        const itemId = String(lootEntry.item_id || '');
        if (itemId && !itemIds.has(itemId)) {
          issues.push({ severity: 'error', area: 'Loot', entryId: entry.id, message: `Unknown item_id "${itemId}".` });
        }
        const chance = number(lootEntry.drop_chance, -1);
        if (chance < 0 || chance > 1) {
          issues.push({ severity: 'error', area: 'Loot', entryId: entry.id, message: `Drop chance for "${itemId || 'entry'}" must be between 0 and 1.` });
        }
      }
    }

    if (entry.kind === 'attack') {
      if (!entry.data.name) {
        issues.push({ severity: 'warning', area: 'Attacks', entryId: entry.id, message: 'Missing name.' });
      }
      if (number(entry.data.cooldown_ms, 0) <= 0) {
        issues.push({ severity: 'error', area: 'Attacks', entryId: entry.id, message: 'cooldown_ms must be greater than 0.' });
      }
      if (number(entry.data.mana_cost, 0) < 0) {
        issues.push({ severity: 'error', area: 'Attacks', entryId: entry.id, message: 'mana_cost cannot be negative.' });
      }
    }
  }

  for (const [key, duplicateEntries] of ids) {
    if (duplicateEntries.length > 1) {
      const id = key.slice(key.indexOf(':') + 1);
      issues.push({
        severity: 'error',
        area: 'IDs',
        entryId: id,
        message: `Defined in multiple files: ${duplicateEntries.map((entry) => entry.file).join(', ')}.`,
      });
    }
  }

  const uniqueSpawnIds = new Map<string, string>();
  for (const chunk of chunks) {
    for (const spawn of chunk.entities) {
      const location = `${chunk.coord.cx},${chunk.coord.cy} (${spawn.x},${spawn.y})`;
      if (!entityIds.has(spawn.entityId)) {
        issues.push({
          severity: 'error',
          area: 'Maps',
          entryId: spawn.entityId,
          message: `Unknown entity spawn at chunk ${location}.`,
        });
      }
      if (spawn.x < 0 || spawn.y < 0 || spawn.x >= chunk.width || spawn.y >= chunk.height) {
        issues.push({
          severity: 'error',
          area: 'Maps',
          entryId: spawn.entityId,
          message: `Spawn is outside chunk bounds at ${location}.`,
        });
      }
      if (spawn.uniqueId) {
        const previous = uniqueSpawnIds.get(spawn.uniqueId);
        if (previous) {
          issues.push({
            severity: 'error',
            area: 'Maps',
            entryId: spawn.uniqueId,
            message: `Duplicate uniqueId at ${previous} and ${location}.`,
          });
        } else {
          uniqueSpawnIds.set(spawn.uniqueId, location);
        }
      }
    }

    if (chunk.layers.ground.every((tile) => tile === 0)) {
      issues.push({
        severity: 'warning',
        area: 'Maps',
        message: `Chunk ${chunk.coord.cx},${chunk.coord.cy} has no ground tiles.`,
      });
    }
  }

  return issues.sort((a, b) => {
    if (a.severity !== b.severity) return a.severity === 'error' ? -1 : 1;
    return a.area.localeCompare(b.area);
  });
}
