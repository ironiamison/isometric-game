import gameData from './game-data.json';
import { STATIC_ARTICLES } from './articles';
import type { GameEntity, GameItem, GameQuest, WikiArticle, WikiGameData, WikiNavGroup, WikiSection } from './types';

const data = gameData as WikiGameData;

function questArticle(q: GameQuest): WikiArticle {
  return {
    slug: q.slug,
    title: q.name,
    summary: q.description,
    section: 'content',
    icon: q.repeatable ? '↻' : '📜',
    updatedAt: '2024-05-01',
    blocks: [
      { type: 'p', text: q.description },
      { type: 'h2', text: 'Details' },
      {
        type: 'ul',
        items: [
          `Quest giver: ${q.giver_npc.replace(/_/g, ' ')}`,
          `Level required: ${q.level_required}`,
          q.repeatable ? 'Repeatable: yes' : 'Repeatable: no',
          `Category: ${q.folder.replace(/_/g, ' ')}`,
          q.exp ? `XP reward: ${q.exp}` : '',
          q.gold ? `Gold reward: ${q.gold}` : '',
        ].filter(Boolean),
      },
    ],
  };
}

function categoryCounts(items: GameItem[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const item of items) {
    counts[item.category] = (counts[item.category] ?? 0) + 1;
  }
  return counts;
}

function itemsArticle(items: GameItem[]): WikiArticle {
  const counts = categoryCounts(items);
  const categorySummary = Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([cat, n]) => `${cat.charAt(0).toUpperCase() + cat.slice(1)}: ${n}`)
    .join(' · ');

  const rows = items
    .map(
      (item) =>
        `<tr><td><strong>${item.display_name}</strong></td><td>${item.category}</td><td>${item.description || '—'}</td><td>${item.base_price > 0 ? item.base_price.toLocaleString() + ' gp' : '—'}</td></tr>`,
    )
    .join('');

  return {
    slug: 'items',
    title: 'Items',
    summary: `Full searchable item database — ${items.length} items.`,
    section: 'content',
    icon: '📦',
    thumbnail: '/wiki/wiki-cat-items.png',
    updatedAt: '2024-05-12',
    popular: true,
    externalLink: '/world/items',
    blocks: [
      {
        type: 'p',
        text: `Every item in Solstead — weapons, armour, tools, consumables, quest items, and materials. ${items.length} entries from game data.`,
      },
      { type: 'h2', text: 'Categories' },
      { type: 'p', text: categorySummary },
      { type: 'h2', text: 'All items' },
      {
        type: 'html',
        html: `<div class="wiki-table-wrap"><table class="wiki-table"><thead><tr><th>Name</th><th>Type</th><th>Description</th><th>Value</th></tr></thead><tbody>${rows}</tbody></table></div>`,
      },
      { type: 'link', href: '/world/items', label: 'Search & filter in item database →' },
    ],
  };
}

function monstersArticle(entities: GameEntity[]): WikiArticle {
  const rows = entities
    .map(
      (e) =>
        `<tr><td><a href="/world/bestiary/${encodeURIComponent(e.id)}">${e.display_name}</a></td><td>Lv ${e.level}</td><td>${e.max_hp}</td><td>${e.damage}</td><td>${e.exp_base * e.level}</td><td>${e.loot.length + e.loot_tables.length}</td></tr>`,
    )
    .join('');

  return {
    slug: 'monsters',
    title: 'Monsters',
    summary: `Bestiary with stats, drops, and scaling — ${entities.length} creatures.`,
    section: 'content',
    icon: '👹',
    thumbnail: '/wiki/wiki-cat-monsters.png',
    updatedAt: '2024-05-12',
    popular: true,
    externalLink: '/world/bestiary',
    blocks: [
      {
        type: 'p',
        text: `Every hostile creature — level, HP, damage, XP, gold, and loot tables. Click any monster for full drop details and related quests.`,
      },
      { type: 'h2', text: 'All monsters' },
      {
        type: 'html',
        html: `<div class="wiki-table-wrap"><table class="wiki-table"><thead><tr><th>Name</th><th>Level</th><th>HP</th><th>Damage</th><th>XP</th><th>Drops</th></tr></thead><tbody>${rows}</tbody></table></div>`,
      },
      { type: 'link', href: '/world/bestiary', label: 'Open searchable bestiary →' },
    ],
  };
}

export const WIKI_STATS = data.stats;

export const ALL_ARTICLES: WikiArticle[] = [
  ...STATIC_ARTICLES.filter((a) => a.slug !== 'quests'),
  itemsArticle(data.items),
  monstersArticle(data.entities),
  ...data.quests.map(questArticle),
  {
    slug: 'quests',
    title: 'Quest Index',
    summary: `All ${data.quests.length} quests in Solstead.`,
    section: 'content',
    icon: '📖',
    updatedAt: '2024-05-12',
    blocks: [
      { type: 'p', text: 'Quests unlock areas, gear, and storylines. Some use Lua scripts for complex dialogue and puzzles.' },
      { type: 'h2', text: 'All quests' },
      {
        type: 'html',
        html: `<ul class="wiki-list">${data.quests.map((q) => `<li><a href="/wiki/${q.slug}">${q.name}</a> — ${q.description}</li>`).join('')}</ul>`,
      },
    ],
  },
  {
    slug: 'interior-index',
    title: 'All Interiors',
    summary: `${data.interiors.length} instanced maps — banks, shops, dungeons, arenas.`,
    section: 'world',
    icon: '🚪',
    updatedAt: '2024-05-12',
    blocks: [
      { type: 'p', text: 'Every instanced interior in the game:' },
      {
        type: 'ul',
        items: data.interiors.map((id) => id.replace(/_/g, ' ')),
      },
    ],
  },
];

export const ARTICLE_BY_SLUG = new Map(ALL_ARTICLES.map((a) => [a.slug, a]));

export const WIKI_NAV: WikiNavGroup[] = [
  {
    id: 'getting-started',
    label: 'Getting Started',
    links: [
      { slug: 'welcome', label: 'Welcome to Solstead', icon: '☀' },
      { slug: 'game-overview', label: 'Game Overview', icon: '📜' },
      { slug: 'controls', label: 'Controls & Interface', icon: '🎮' },
      { slug: 'new-player-guide', label: 'New Player Guide', icon: '🌱' },
    ],
  },
  {
    id: 'world',
    label: 'World',
    links: [
      { slug: 'world-of-solstead', label: 'The World of Solstead', icon: '🌍' },
      { slug: 'regions', label: 'Regions', icon: '🏰' },
      { slug: 'towns', label: 'Towns & Cities', icon: '🏘' },
      { slug: 'points-of-interest', label: 'Points of Interest', icon: '📍' },
      { slug: 'dungeons', label: 'Dungeons', icon: '🕳' },
      { slug: 'biomes', label: 'Biomes & Terrain', icon: '🌲' },
    ],
  },
  {
    id: 'gameplay',
    label: 'Gameplay',
    links: [
      { slug: 'skills', label: 'Skills', icon: '✦' },
      { slug: 'combat', label: 'Combat', icon: '⚔' },
      { slug: 'professions', label: 'Professions', icon: '⛏' },
      { slug: 'clans', label: 'Clans', icon: '🛡' },
      { slug: 'economy', label: 'Economy', icon: '🪙' },
      { slug: 'death-and-loss', label: 'Death & Loss', icon: '💀' },
    ],
  },
  {
    id: 'content',
    label: 'Content',
    links: [
      { slug: 'items', label: 'Items', icon: '📦' },
      { slug: 'monsters', label: 'Monsters', icon: '👹' },
      { slug: 'bosses', label: 'Bosses', icon: '🐉' },
      { slug: 'resources', label: 'Resources', icon: '🌾' },
      { slug: 'achievements', label: 'Achievements', icon: '🏆' },
      { slug: 'quests', label: 'Quest Index', icon: '📖' },
    ],
  },
  {
    id: 'community',
    label: 'Community',
    links: [
      { slug: 'rules', label: 'Rules', icon: '📋' },
      { slug: 'player-conduct', label: 'Player Conduct', icon: '🤝' },
      { slug: 'support', label: 'Support', icon: '❓' },
    ],
  },
];

export const EXPLORE_TILES = [
  { slug: 'regions', label: 'Regions', desc: 'Explore the lands of Solstead.', img: '/wiki/wiki-cat-regions.png' },
  { slug: 'combat', label: 'Combat', desc: 'Master combat skills and slayer.', img: '/wiki/wiki-cat-combat.png' },
  { slug: 'professions', label: 'Professions', desc: 'Gather, craft, and trade.', img: '/wiki/wiki-cat-professions.png' },
  { slug: 'items', label: 'Items', desc: 'Browse the full item database.', img: '/wiki/wiki-cat-items.png' },
  { slug: 'monsters', label: 'Monsters', desc: 'Study creatures and loot.', img: '/wiki/wiki-cat-monsters.png' },
  { slug: 'dungeons', label: 'Dungeons', desc: 'Delve into instanced content.', img: '/wiki/wiki-cat-dungeons.png' },
];

export function searchArticles(query: string, limit = 20): WikiArticle[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  return ALL_ARTICLES.filter(
    (a) =>
      a.title.toLowerCase().includes(q) ||
      a.summary.toLowerCase().includes(q) ||
      a.slug.includes(q),
  ).slice(0, limit);
}

export function popularArticles(): WikiArticle[] {
  return ALL_ARTICLES.filter((a) => a.popular);
}

export function recentArticles(): WikiArticle[] {
  return [...ALL_ARTICLES]
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 6);
}

export function recentlyUpdated(): WikiArticle[] {
  return [...STATIC_ARTICLES]
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .slice(0, 5);
}

export function articlesInSection(section: WikiSection): WikiArticle[] {
  return ALL_ARTICLES.filter((a) => a.section === section);
}

export function formatDate(iso: string): string {
  const d = new Date(iso + 'T12:00:00');
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}
