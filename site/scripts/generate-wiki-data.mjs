#!/usr/bin/env node
/** Extract quest, item, and entity data from rust-server for the wiki and world pages. */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import TOML from 'smol-toml';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '../..');
const DATA = path.join(ROOT, 'rust-server/data');
const OUT = path.join(__dirname, '../src/lib/wiki/game-data.json');

function readTomlField(text, field) {
  const re = new RegExp(`^${field}\\s*=\\s*"([^"]*)"`, 'm');
  const m = text.match(re);
  return m?.[1] ?? null;
}

function readTomlBool(text, field) {
  const re = new RegExp(`^${field}\\s*=\\s*(true|false)`, 'm');
  const m = text.match(re);
  return m?.[1] === 'true';
}

function readTomlInt(text, field) {
  const re = new RegExp(`^${field}\\s*=\\s*(\\d+)`, 'm');
  const m = text.match(re);
  return m ? Number(m[1]) : 0;
}

function walkTomlFiles(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc;
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) walkTomlFiles(p, acc);
    else if (ent.name.endsWith('.toml')) acc.push(p);
  }
  return acc;
}

function walkQuests(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc;
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) walkQuests(p, acc);
    else if (ent.name.endsWith('.toml')) {
      const raw = fs.readFileSync(p, 'utf8');
      const id = readTomlField(raw, 'id');
      if (!id) continue;
      acc.push({
        id,
        slug: `quest-${id.replace(/_/g, '-')}`,
        name: readTomlField(raw, 'name') ?? id,
        description: readTomlField(raw, 'description') ?? '',
        giver_npc: readTomlField(raw, 'giver_npc') ?? '',
        level_required: readTomlInt(raw, 'level_required'),
        repeatable: readTomlBool(raw, 'repeatable'),
        folder: path.relative(path.join(DATA, 'quests'), p).split(path.sep)[0],
        exp: readTomlInt(raw, 'exp'),
        gold: readTomlInt(raw, 'gold'),
      });
    }
  }
  return acc;
}

function buildQuestKillMap() {
  const map = new Map();
  for (const file of walkTomlFiles(path.join(DATA, 'quests'))) {
    let parsed;
    try {
      parsed = TOML.parse(fs.readFileSync(file, 'utf8'));
    } catch {
      continue;
    }
    const quest = parsed.quest;
    if (!quest?.id || !Array.isArray(quest.objectives)) continue;
    for (const obj of quest.objectives) {
      if (obj.type === 'kill_monster' && obj.target) {
        const list = map.get(obj.target) ?? [];
        if (!list.includes(quest.id)) list.push(quest.id);
        map.set(obj.target, list);
      }
    }
  }
  return map;
}

function parseEquipment(raw) {
  if (!raw || raw.slot_type === 'none') return null;
  return {
    slot_type: raw.slot_type ?? 'none',
    attack_level_required: raw.attack_level_required ?? 1,
    defence_level_required: raw.defence_level_required ?? 1,
    ranged_level_required: raw.ranged_level_required ?? 0,
    woodcutting_level_required: raw.woodcutting_level_required ?? 1,
    mining_level_required: raw.mining_level_required ?? 1,
    magic_level_required: raw.magic_level_required ?? 0,
    attack_bonus: raw.attack_bonus ?? 0,
    strength_bonus: raw.strength_bonus ?? 0,
    defence_bonus: raw.defence_bonus ?? 0,
    ranged_strength_bonus: raw.ranged_strength_bonus ?? 0,
    magic_bonus: raw.magic_bonus ?? 0,
    weapon_type: raw.weapon_type ?? 'melee',
    range: raw.range ?? 1,
  };
}

function loadItems() {
  const items = [];
  for (const file of walkTomlFiles(path.join(DATA, 'items'))) {
    const table = TOML.parse(fs.readFileSync(file, 'utf8'));
    for (const [id, raw] of Object.entries(table)) {
      items.push({
        id,
        display_name: raw.display_name ?? id,
        sprite: raw.sprite ?? id,
        description: raw.description ?? '',
        category: raw.category ?? 'material',
        max_stack: raw.max_stack ?? 1,
        base_price: raw.base_price ?? 0,
        sellable: raw.sellable ?? false,
        equipment: parseEquipment(raw.equipment),
      });
    }
  }
  return items.sort((a, b) => a.display_name.localeCompare(b.display_name));
}

function loadEntities(questKillMap) {
  const entities = [];
  const monstersDir = path.join(DATA, 'entities/monsters');
  for (const file of walkTomlFiles(monstersDir)) {
    const table = TOML.parse(fs.readFileSync(file, 'utf8'));
    for (const [id, raw] of Object.entries(table)) {
      if (!raw.behaviors?.hostile) continue;
      const stats = raw.stats ?? {};
      const rewards = raw.rewards ?? {};
      entities.push({
        id,
        display_name: raw.display_name ?? id,
        sprite: raw.sprite ?? id,
        description: raw.description ?? '',
        level: stats.level ?? 1,
        max_hp: stats.max_hp ?? 1,
        damage: stats.damage ?? 1,
        attack_bonus: stats.attack_bonus ?? 0,
        defence_bonus: stats.defence_bonus ?? 0,
        attack_range: stats.attack_range ?? 1,
        aggro_range: stats.aggro_range ?? 4,
        respawn_time_ms: stats.respawn_time_ms ?? 10000,
        hostile: raw.behaviors.hostile ?? true,
        exp_base: rewards.exp_base ?? 0,
        gold_min: rewards.gold_min ?? 0,
        gold_max: rewards.gold_max ?? 0,
        loot: (raw.loot ?? []).map((l) => ({
          item_id: l.item_id,
          drop_chance: l.drop_chance ?? 0,
          quantity_min: l.quantity_min ?? 1,
          quantity_max: l.quantity_max ?? 1,
        })),
        loot_tables: (raw.loot_tables ?? []).map((t) => ({
          name: t.name ?? 'loot',
          chance: t.chance ?? 1,
          entries: (t.entries ?? []).map((e) => ({
            item_id: e.item_id,
            weight: e.weight ?? 1,
            quantity_min: e.quantity_min ?? 1,
            quantity_max: e.quantity_max ?? 1,
          })),
        })),
        quest_ids: questKillMap.get(id) ?? [],
      });
    }
  }
  return entities.sort((a, b) => a.level - b.level || a.display_name.localeCompare(b.display_name));
}

function countRecipes() {
  let n = 0;
  const dir = path.join(DATA, 'recipes');
  for (const f of fs.readdirSync(dir).filter((x) => x.endsWith('.toml'))) {
    const raw = fs.readFileSync(path.join(dir, f), 'utf8');
    n += (raw.match(/^\[\[/gm) ?? []).length;
  }
  return n;
}

function listInteriors() {
  const dir = path.join(ROOT, 'rust-server/maps/interiors');
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith('.json'))
    .map((f) => f.replace('.json', ''))
    .sort();
}

function listShops() {
  const dir = path.join(DATA, 'shops');
  return fs.readdirSync(dir).filter((f) => f.endsWith('.toml')).map((f) => f.replace('.toml', ''));
}

const questKillMap = buildQuestKillMap();
const items = loadItems();
const entities = loadEntities(questKillMap);

const payload = {
  generated_at: new Date().toISOString(),
  stats: {
    quests: walkQuests(path.join(DATA, 'quests')).length,
    items: items.length,
    entities: entities.length,
    recipes: countRecipes(),
    shops: listShops().length,
    interiors: listInteriors().length,
  },
  quests: walkQuests(path.join(DATA, 'quests')),
  items,
  entities,
  interiors: listInteriors(),
  shops: listShops(),
};

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify(payload, null, 2));
console.log('Wiki game-data written:', OUT, payload.stats);
