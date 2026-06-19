import { interiorHighlights, mapPois, worldActivities, worldRegions } from '$lib/world-guide';
import type { WikiArticle } from './types';

const regionsList = worldRegions
  .map((r) => `<li><strong>${r.name}</strong> — ${r.tagline}. ${r.description}</li>`)
  .join('');

const activitiesList = worldActivities
  .map((a) => `<li><strong>${a.title}</strong> — ${a.summary}</li>`)
  .join('');

const poiList = mapPois
  .map((p) => `<li><strong>${p.name}</strong> (${p.kind}) — ${p.blurb}</li>`)
  .join('');

const interiorList = interiorHighlights
  .map((i) => `<li><strong>${i.name}</strong> — ${i.note}</li>`)
  .join('');

export const STATIC_ARTICLES: WikiArticle[] = [
  {
    slug: 'welcome',
    title: 'Welcome to Solstead',
    summary: 'Start here — what Solstead is and how this wiki helps.',
    section: 'getting-started',
    icon: '☀',
    updatedAt: '2024-05-01',
    popular: true,
    blocks: [
      { type: 'p', text: 'Solstead is a persistent online isometric MMO — gather, craft, quest, trade, and explore a shared world with other players. This wiki is built from live game data and design docs.' },
      { type: 'h2', text: 'Quick links' },
      { type: 'ul', items: ['New Player Guide — tutorial quests and first hours', 'Skills — all 16 trainable skills', 'World Map — interactive overworld at /world', 'Item Database — every item at /world/items', 'Bestiary — every monster at /world/bestiary'] },
    ],
  },
  {
    slug: 'game-overview',
    title: 'Game Overview',
    summary: 'Core loops: combat, skilling, quests, and economy.',
    section: 'getting-started',
    icon: '📜',
    updatedAt: '2024-05-02',
    popular: true,
    blocks: [
      { type: 'p', text: 'Solstead follows classic MMO skill progression (RuneScape-inspired): train combat and gathering skills to level 99, complete quest chains for unlocks, and participate in a player-driven economy with NPC shops and banks.' },
      { type: 'h2', text: 'Core pillars' },
      { type: 'ul', items: worldActivities.map((a) => `${a.title} — ${a.summary}`) },
    ],
  },
  {
    slug: 'controls',
    title: 'Controls & Interface',
    summary: 'Movement, combat, inventory, and UI basics.',
    section: 'getting-started',
    icon: '🎮',
    updatedAt: '2024-05-03',
    blocks: [
      { type: 'p', text: 'Click to move in the isometric world. Interact with NPCs, objects, and other players by clicking them. Combat is real-time — target enemies and use your equipped weapon or spells.' },
      { type: 'h2', text: 'Key interfaces' },
      { type: 'ul', items: ['Inventory & equipment — manage gear and tools', 'Skills panel — track all 16 skills and XP', 'Quest journal — active objectives and dialogue', 'World map — ports, dungeons, and landmarks (in-game)', 'Bank — store items and gold safely in towns'] },
    ],
  },
  {
    slug: 'new-player-guide',
    title: "New Player Guide",
    summary: 'Your first hour: tutorial quests, tools, and New Aeven.',
    section: 'getting-started',
    icon: '🌱',
    updatedAt: '2024-05-04',
    popular: true,
    blocks: [
      { type: 'p', text: 'New characters spawn in Verdant Fields. Complete the tutorial quest chain to learn mining, farming, and woodcutting, then push into New Aeven for banking, shops, and The Awakening storyline.' },
      { type: 'h2', text: 'Tutorial quests (recommended order)' },
      { type: 'ul', items: ['Rock Bottom — mining copper with Miner Mike', 'Green Thumb — farming basics', 'Axe to Grind — woodcutting introduction', 'Tools of Trade — survivalist crafting intro', 'Leather Craft & Warm Meal — cooking and leatherworking'] },
      { type: 'h2', text: 'First goals' },
      { type: 'ul', items: ['Reach New Aeven and unlock the bank', 'Train combat on overworld monsters', 'Try fishing at a pond (Lv.1) or river (Lv.15)', 'Talk to quest NPCs — yellow indicators mark givers'] },
    ],
  },
  {
    slug: 'world-of-solstead',
    title: 'The World of Solstead',
    summary: 'One persistent overworld plus instanced interiors.',
    section: 'world',
    icon: '🌍',
    updatedAt: '2024-05-05',
    popular: false,
    blocks: [
      { type: 'p', text: 'The overworld is built from hundreds of handcrafted map chunks covering grasslands, coasts, desert, swamp, and northern wilds. Dungeons, banks, shops, and arenas are instanced interiors reached through doors and portals.' },
      { type: 'h2', text: 'Explore the map' },
      { type: 'link', href: '/world', label: 'Open interactive world map →' },
    ],
  },
  {
    slug: 'regions',
    title: 'Regions',
    summary: 'All eight major overworld regions.',
    section: 'world',
    icon: '🏰',
    thumbnail: '/wiki/wiki-cat-regions.png',
    updatedAt: '2024-05-12',
    popular: true,
    blocks: [
      { type: 'p', text: 'Each region has distinct biomes, monsters, hubs, and activities.' },
      { type: 'html', html: `<ul class="wiki-list">${regionsList}</ul>` },
    ],
  },
  {
    slug: 'towns',
    title: 'Towns & Cities',
    summary: 'Major hubs: New Aeven, Oakshore, and desert tents.',
    section: 'world',
    icon: '🏘',
    updatedAt: '2024-05-06',
    blocks: [
      { type: 'h2', text: 'New Aeven' },
      { type: 'p', text: 'Southern capital hub — bank, blacksmith, tailor, guard house, church, and port. Main quest line The Awakening starts here.' },
      { type: 'h2', text: 'Oakshore' },
      { type: 'p', text: 'Eastern coastal region — farming allotments, magic shop, slayer caves, church, and Oakshore Port.' },
      { type: 'h2', text: 'Desert camps' },
      { type: 'p', text: 'Scorching Sands — jewel shop, desert bank, slayer master, fishing cave, and pyramid tomb entrance.' },
    ],
  },
  {
    slug: 'points-of-interest',
    title: 'Points of Interest',
    summary: 'Ports, waystones, dungeons, and travel routes.',
    section: 'world',
    icon: '📍',
    updatedAt: '2024-05-07',
    blocks: [
      { type: 'html', html: `<ul class="wiki-list">${poiList}</ul>` },
      { type: 'link', href: '/world', label: 'View on world map →' },
    ],
  },
  {
    slug: 'dungeons',
    title: 'Dungeons & Interiors',
    summary: 'Instanced combat, puzzles, and boss content.',
    section: 'world',
    icon: '🕳',
    thumbnail: '/wiki/wiki-cat-dungeons.png',
    updatedAt: '2024-05-11',
    popular: false,
    blocks: [
      { type: 'p', text: 'Step through portals to reach instanced areas. Major dungeons include combat zones, puzzle rooms, and boss encounters.' },
      { type: 'h2', text: 'Notable interiors' },
      { type: 'html', html: `<ul class="wiki-list">${interiorList}</ul>` },
      { type: 'p', text: 'See the Dungeons & Bosses article for combat details and the full interior list (52 instanced maps).' },
    ],
  },
  {
    slug: 'biomes',
    title: 'Biomes & Terrain',
    summary: 'Grassland, coast, desert, swamp, and northern wilds.',
    section: 'world',
    icon: '🌲',
    updatedAt: '2024-05-08',
    blocks: [
      { type: 'ul', items: ['Verdant Fields — starter grasslands and oak trees', 'Oakshore — coastal farms and rivers', 'Scorching Sands — desert ores, wurms, and tents', 'Murkwood Swamp — herbs, spiders, witch houses', 'Northern Reaches — obelisks and reaper dungeons', 'Wilderness — PvP-enabled frontier zones'] },
    ],
  },
  {
    slug: 'skills',
    title: 'Skills',
    summary: 'All 16 trainable skills to level 99.',
    section: 'gameplay',
    icon: '✦',
    updatedAt: '2024-05-09',
    popular: true,
    blocks: [
      { type: 'h2', text: 'Combat' },
      { type: 'ul', items: ['Hitpoints — max HP (starts at 10)', 'Attack — melee accuracy', 'Strength — melee max hit', 'Defence — evasion', 'Ranged — bows and crossbows', 'Prayer — combat buffs, drains over time', 'Magic — spellcasting (10 spells + scrolls)', 'Slayer — task-based hunting from masters'] },
      { type: 'h2', text: 'Gathering' },
      { type: 'ul', items: ['Woodcutting — oak through yew trees', 'Mining — copper through runite ores', 'Fishing — ponds, rivers, ocean', 'Farming — crops and herb gathering'] },
      { type: 'h2', text: 'Production' },
      { type: 'ul', items: ['Smithing — bars, weapons, armour at anvil/furnace', 'Alchemy — potions at alchemy station', 'Survivalist — cooking, fletching, leatherworking XP'] },
    ],
  },
  {
    slug: 'combat',
    title: 'Combat',
    summary: 'Melee, ranged, magic, prayers, and slayer.',
    section: 'gameplay',
    icon: '⚔',
    thumbnail: '/wiki/wiki-cat-combat.png',
    updatedAt: '2024-05-10',
    popular: true,
    blocks: [
      { type: 'p', text: 'Combat uses attack vs defence rolls and strength for max hit. Monsters scale by level — higher zones mean tougher foes and better loot.' },
      { type: 'h2', text: 'Combat styles' },
      { type: 'ul', items: ['Melee — sword/axe with Attack + Strength + Defence', 'Ranged — bows requiring Ranged level and ammo', 'Magic — spells costing mana with Magic level gates', 'Prayer — toggle buffs like Thick Skin, Burst of Strength'] },
      { type: 'h2', text: 'Slayer' },
      { type: 'p', text: 'Slayer masters assign task monsters. Complete tasks for points spent at slayer reward shops. Masters in Oakshore and Desert caves.' },
      { type: 'link', href: '/world/bestiary', label: 'Browse full bestiary →' },
    ],
  },
  {
    slug: 'professions',
    title: 'Professions & Crafting',
    summary: 'Gathering, recipes, stations, and crafting orders.',
    section: 'gameplay',
    icon: '⛏',
    thumbnail: '/wiki/wiki-cat-professions.png',
    updatedAt: '2024-05-08',
    popular: true,
    blocks: [
      { type: 'p', text: 'Gather raw materials from the world, process them at crafting stations, and sell or equip the results. Over 300 recipes across smithing, alchemy, cooking, fletching, and leatherworking.' },
      { type: 'h2', text: 'Crafting stations' },
      { type: 'ul', items: ['Anvil — smith weapons and armour', 'Furnace — smelt ore into bars', 'Workbench — fletching and leather', 'Alchemy station — potions', 'Fire pit / cooker — food'] },
      { type: 'h2', text: 'Crafting orders' },
      { type: 'p', text: 'Profession delivery contracts reward gold and XP — talk to order NPCs in hubs for rotating jobs (mining, woodcutting, fishing, smithing, alchemy).' },
    ],
  },
  {
    slug: 'clans',
    title: 'Clans',
    summary: 'Guild systems and group play.',
    section: 'gameplay',
    icon: '🛡',
    updatedAt: '2024-05-05',
    blocks: [
      { type: 'p', text: 'Formal clan/guild systems are planned. Currently, players party up organically in shared overworld instances, duel in arenas, and compete on leaderboards. King of the Hill and wilderness PvP support group conflict.' },
    ],
  },
  {
    slug: 'economy',
    title: 'Economy',
    summary: 'Gold, shops, banks, and trading.',
    section: 'gameplay',
    icon: '🪙',
    updatedAt: '2024-05-07',
    blocks: [
      { type: 'p', text: 'Gold drops from monsters and quest rewards. NPC shops buy and sell with restock timers. Banks in every major hub store items and gold safely.' },
      { type: 'h2', text: 'Shop types' },
      { type: 'ul', items: ['General stores — basics and sell-anything', 'Blacksmith — weapons and armour', 'Magic shop — staves and runes', 'Jewel shop — desert accessories', 'Slayer rewards — task point shop'] },
      { type: 'p', text: 'Face-to-face player trading is supported in safe zones.' },
    ],
  },
  {
    slug: 'death-and-loss',
    title: 'Death & Loss',
    summary: 'What happens when you die.',
    section: 'gameplay',
    icon: '💀',
    updatedAt: '2024-05-10',
    blocks: [
      { type: 'p', text: 'In safe zones (towns and most overworld), death typically respawns you at a nearby point with minimal penalty. In the Wilderness PvP zone, other players can attack you and you may lose items — enter prepared.' },
      { type: 'ul', items: ['Bank valuable gear before wilderness runs', 'Eat food to restore HP in combat', 'Use Return Home spell (Magic) to escape', 'Prayer protection helps in tough fights'] },
    ],
  },
  {
    slug: 'bosses',
    title: 'Bosses',
    summary: 'Pharaoh, Desert Wurm, Reaper, Poltergeist, and more.',
    section: 'content',
    icon: '🐉',
    updatedAt: '2024-05-11',
    blocks: [
      { type: 'ul', items: ['Pharaoh — Pyramid Tomb (Pharaoh\'s Curse quest)', 'Desert Wurm — boss cave in Scorching Sands', 'Haunted Poltergeist — Haunted House dungeon', 'Reaper — northern reaper dungeons', 'Pig King — Oakshore slayer content', 'King of the Hill — instanced PvP boss waves'] },
      { type: 'link', href: '/world/bestiary', label: 'Boss stats in bestiary →' },
    ],
  },
  {
    slug: 'resources',
    title: 'Resources',
    summary: 'Trees, ores, fish, herbs, and farming crops.',
    section: 'content',
    icon: '🌾',
    updatedAt: '2024-05-09',
    blocks: [
      { type: 'h2', text: 'Woodcutting tiers' },
      { type: 'ul', items: ['Oak (Lv.1) — Verdant Fields', 'Willow (Lv.15) — riversides', 'Maple (Lv.45) — mid-level zones', 'Yew (Lv.60) — endgame trees'] },
      { type: 'h2', text: 'Mining & fishing' },
      { type: 'ul', items: ['Copper/tin near starter areas', 'Iron, coal, mithril, adamant, runite at higher levels', 'Ponds Lv.1, rivers Lv.15, ocean Lv.40'] },
      { type: 'h2', text: 'Farming' },
      { type: 'p', text: 'Allotment plots near Oakshore — plant seeds, wait for growth timers, harvest for cooking and Farming XP.' },
    ],
  },
  {
    slug: 'achievements',
    title: 'Achievements & Collection Log',
    summary: 'Long-term goals and rare drops.',
    section: 'content',
    icon: '🏆',
    updatedAt: '2024-05-06',
    blocks: [
      { type: 'p', text: 'The collection log tracks rare drops from monsters, bosses, skills, and quests. Completing sets is a long-term prestige goal (see CONTENT_ROADMAP Phase 5).' },
      { type: 'ul', items: ['Monster drops — unique loot per creature', 'Boss trophies — pharaoh, reaper, wurm items', 'Skill milestones — level 99 capes planned', 'Quest rewards — unique gear and titles'] },
    ],
  },
  {
    slug: 'quests',
    title: 'Quest Index',
    summary: 'All 28 quests in Solstead.',
    section: 'content',
    icon: '📖',
    updatedAt: '2024-05-12',
    blocks: [
      { type: 'p', text: 'Quests unlock areas, gear, and storylines. Some use Lua scripts for complex dialogue and puzzles.' },
      { type: 'h2', text: 'Quest families' },
      { type: 'ul', items: ['Tutorial (3) — mining, farming, woodcutting', 'Survivalist (3) — tools, leather, cooking', 'Cursed Lands (2) — early area intro', 'Swamp (6) — Murkwood storyline', 'The Awakening (5) — main New Aeven arc', 'Exploration — Obelisk Connection', 'Pharaoh\'s Curse — desert tomb', 'Ghastly Contraption — Haunted House', 'Adventurer tiers (3) — progression gates', 'Repeatables (3) — daily-style jobs'] },
    ],
  },
  {
    slug: 'rules',
    title: 'Rules',
    summary: 'Fair play and community standards.',
    section: 'community',
    icon: '📋',
    updatedAt: '2024-05-01',
    blocks: [
      { type: 'ul', items: ['No cheating, botting, or exploiting bugs', 'Respect other players — no harassment', 'Do not scam or manipulate trades', 'Report issues to the team via Discord'] },
    ],
  },
  {
    slug: 'player-conduct',
    title: 'Player Conduct',
    summary: 'Chat, trading, and PvP etiquette.',
    section: 'community',
    icon: '🤝',
    updatedAt: '2024-05-02',
    blocks: [
      { type: 'p', text: 'Solstead is a social MMO. Keep chat constructive, honor trade agreements, and understand wilderness rules before entering PvP zones.' },
    ],
  },
  {
    slug: 'support',
    title: 'Support',
    summary: 'Get help and report bugs.',
    section: 'community',
    icon: '❓',
    updatedAt: '2024-05-01',
    blocks: [
      { type: 'p', text: 'Join the community Discord for help, bug reports, and updates. For account issues, contact the team through official channels.' },
      { type: 'link', href: 'https://discord.gg/solstead', label: 'Join Discord →' },
    ],
  },
];
