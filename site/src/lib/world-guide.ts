/** Solstead world guide — regions, POIs, and activities (from game data). */

export type MapPoi = {
  id: string;
  name: string;
  /** 0–100 percent on map image */
  x: number;
  y: number;
  kind: 'port' | 'town' | 'dungeon' | 'landmark' | 'travel' | 'pvp' | 'resource';
  regionId: string;
  blurb: string;
  /** Approximate in-game tile coords for the location readout */
  tileX?: number;
  tileY?: number;
};

export type HubMarker = {
  id: string;
  name: string;
  x: number;
  y: number;
  kind: 'bank' | 'shop';
  regionId: string;
  tileX?: number;
  tileY?: number;
};

export type MapFilterId =
  | 'towns'
  | 'cities'
  | 'dungeons'
  | 'poi'
  | 'resources'
  | 'banks'
  | 'shops'
  | 'docks'
  | 'safe';

export const defaultMapFilters: Record<MapFilterId, boolean> = {
  towns: true,
  cities: true,
  dungeons: true,
  poi: true,
  resources: true,
  banks: true,
  shops: true,
  docks: true,
  safe: true,
};

export const mapLegendItems: {
  filterId?: MapFilterId;
  label: string;
  icon: string;
  color?: string;
  static?: boolean;
}[] = [
  { label: 'You', icon: '◎', color: '#4a9fd4', static: true },
  { filterId: 'towns', label: 'Town', icon: '🏠', color: '#c4a882' },
  { filterId: 'cities', label: 'City', icon: '🏰', color: '#d4a844' },
  { filterId: 'dungeons', label: 'Dungeon', icon: '🕳', color: '#c45a3a' },
  { filterId: 'poi', label: 'Point of Interest', icon: '◆', color: '#e8c84a' },
  { filterId: 'resources', label: 'Resource Node', icon: '⛏', color: '#7a9a5f' },
  { filterId: 'banks', label: 'Bank', icon: '🪙', color: '#d4a844' },
  { filterId: 'shops', label: 'Shop', icon: '🛍', color: '#a88850' },
  { filterId: 'docks', label: 'Dock', icon: '⚓', color: '#5a8aaa' },
  { filterId: 'safe', label: 'Safe Zone', icon: '🛡', color: '#7a9a5f' },
];

/** Default spawn area — Verdant Fields */
export const playerMapPosition = { x: 58, y: 48, tileX: 0, tileY: 0 };

/** Tile-space bounds used to normalize POI positions on the map art. */
const MAP_BOUNDS = { minX: -450, maxX: 250, minY: -280, maxY: 80 };

export function mapPercentToTile(x: number, y: number): { tileX: number; tileY: number } {
  const tileX = Math.round(MAP_BOUNDS.minX + (x / 100) * (MAP_BOUNDS.maxX - MAP_BOUNDS.minX));
  const tileY = Math.round(MAP_BOUNDS.minY + (y / 100) * (MAP_BOUNDS.maxY - MAP_BOUNDS.minY));
  return { tileX, tileY };
}

const CITY_REGIONS = new Set(['new_aeven', 'oakshore', 'desert']);
const TOWN_REGIONS = new Set(['verdant', 'swamp', 'north', 'deep_desert', 'wilderness']);

export function poiMatchesFilter(poi: MapPoi, filters: Record<MapFilterId, boolean>): boolean {
  if (poi.kind === 'pvp' && filters.safe) return false;
  if (poi.kind === 'port') return filters.docks;
  if (poi.kind === 'dungeon') return filters.dungeons;
  if (poi.kind === 'landmark') return filters.poi;
  if (poi.kind === 'resource') return filters.resources;
  if (poi.kind === 'travel') return filters.poi;
  if (poi.kind === 'town') return filters.towns;
  if (poi.kind === 'pvp') return filters.dungeons || filters.poi;
  return true;
}

export function regionLabelVisible(regionId: string, filters: Record<MapFilterId, boolean>): boolean {
  if (CITY_REGIONS.has(regionId)) return filters.cities;
  if (TOWN_REGIONS.has(regionId)) return filters.towns;
  return filters.cities || filters.towns;
}

export function hubMatchesFilter(hub: HubMarker, filters: Record<MapFilterId, boolean>): boolean {
  if (hub.kind === 'bank') return filters.banks;
  return filters.shops;
}

export type WorldRegion = {
  id: string;
  name: string;
  tagline: string;
  description: string;
  activities: string[];
};

export type WorldActivity = {
  id: string;
  title: string;
  icon: string;
  summary: string;
  details: string[];
};

export type MapRegionLabel = {
  regionId: string;
  /** 0–100 on map image */
  x: number;
  y: number;
};

/** Positions tuned for site/static/world/world-map.png */
export const mapRegionLabels: MapRegionLabel[] = [
  { regionId: 'verdant', x: 58, y: 48 },
  { regionId: 'oakshore', x: 74, y: 36 },
  { regionId: 'new_aeven', x: 48, y: 78 },
  { regionId: 'desert', x: 22, y: 62 },
  { regionId: 'deep_desert', x: 8, y: 48 },
  { regionId: 'swamp', x: 28, y: 22 },
  { regionId: 'north', x: 52, y: 14 },
  { regionId: 'wilderness', x: 68, y: 18 },
];

export const poiKindIcons: Record<MapPoi['kind'], string> = {
  port: '⚓',
  town: '🏰',
  dungeon: '🕳',
  landmark: '✦',
  travel: '🐫',
  pvp: '⚔',
  resource: '🌾',
};

export const poiKindLabels: Record<MapPoi['kind'], string> = {
  port: 'Port',
  town: 'Town',
  dungeon: 'Dungeon',
  landmark: 'Landmark',
  travel: 'Travel',
  pvp: 'PvP Zone',
  resource: 'Resource',
};

export function getRegion(id: string) {
  return worldRegions.find((r) => r.id === id);
}

export function getRegionCenter(regionId: string) {
  return mapRegionLabels.find((l) => l.regionId === regionId);
}

export function getPoi(id: string) {
  return mapPois.find((p) => p.id === id);
}

/** Tile-space bounds used to normalize POI positions on the map art. */
export function tileToMapPercent(x: number, y: number): { x: number; y: number } {
  const px = ((x - MAP_BOUNDS.minX) / (MAP_BOUNDS.maxX - MAP_BOUNDS.minX)) * 100;
  const py = ((y - MAP_BOUNDS.minY) / (MAP_BOUNDS.maxY - MAP_BOUNDS.minY)) * 100;
  return { x: Math.min(98, Math.max(2, px)), y: Math.min(96, Math.max(4, py)) };
}

export const worldRegions: WorldRegion[] = [
  {
    id: 'verdant',
    name: 'Verdant Fields',
    tagline: 'Where every journey begins',
    description:
      'The heart of the overworld — rolling grasslands, rivers, and starter woodcutting spots. Most new players learn combat, gathering, and travel here before pushing into harsher regions.',
    activities: ['Woodcutting oak trees', 'Beginner fishing', 'Starter quests', 'Herb gathering'],
  },
  {
    id: 'oakshore',
    name: 'Oakshore',
    tagline: 'Farms, faith, and the eastern docks',
    description:
      'A green coastal region with allotments, churches, magic shops, and slayer caves. Oakshore Port links the east to New Aeven and the Desert by ship.',
    activities: ['Farming patches', 'Slayer tasks', 'Magic gear shops', 'Reaper boss content'],
  },
  {
    id: 'new_aeven',
    name: 'New Aeven',
    tagline: 'Southern hub of trade and trouble',
    description:
      'The largest town hub — banks, blacksmiths, tailors, guard house, and church. Main quest lines like The Awakening unfold in the streets and cisterns nearby.',
    activities: ['Banking & trading', 'Smithing & tailoring', 'Story quests', 'Ship travel'],
  },
  {
    id: 'desert',
    name: 'Scorching Sands',
    tagline: 'Jewels, wurms, and witchcraft',
    description:
      'Arid wastes with desert tents, jewelers, slayer masters, fishing caves, and the pyramid tomb. Camel routes reach deeper wasteland and forbidden caves.',
    activities: ['Desert slayer', 'Boss caves', 'Camel travel', 'Pharaoh quest line'],
  },
  {
    id: 'deep_desert',
    name: 'Deep Desert',
    tagline: 'Far west — high risk, high reward',
    description:
      'Remote weapon and range shops, a bank, and deadly creatures. Reached only by camel from the main desert for a steep fare.',
    activities: ['High-level gear shops', 'Endgame hunting', 'Long-distance travel'],
  },
  {
    id: 'swamp',
    name: 'Murkwood Swamp',
    tagline: 'Witches, spiders, and starter shelter',
    description:
      'Dark wetlands with witch houses, spider zones, swamp slayer content, and a starter bank. Herb gathering here needs more Farming levels.',
    activities: ['Swamp herbs', 'Spider dungeon', 'Swamp quests', 'Hairdresser & treeseeds'],
  },
  {
    id: 'north',
    name: 'Northern Reaches',
    tagline: 'Obelisks and frozen frontiers',
    description:
      'Northern wilds linked by waystones once you complete the obelisk quest. Reaper dungeons and wilderness content lie off the beaten path.',
    activities: ['Waystone teleport', 'Reaper dungeons', 'Exploration quests'],
  },
  {
    id: 'wilderness',
    name: 'Wilderness',
    tagline: 'PvP-enabled frontier',
    description:
      'Lawless pockets where players can fight each other. The wilderness cabin is a known PvP hotspot — enter only if you are ready to lose your inventory.',
    activities: ['Player vs player combat', 'High-risk looting', 'Remote cabin'],
  },
];

export const mapPois: MapPoi[] = [
  {
    id: 'oakshore_port',
    name: 'Oakshore Port',
    ...tileToMapPercent(37, 54),
    tileX: 37,
    tileY: 54,
    kind: 'port',
    regionId: 'oakshore',
    blurb: 'Eastern docks — sail to New Aeven (50g) or the Desert (250g).',
  },
  {
    id: 'new_aeven_port',
    name: 'New Aeven Port',
    ...tileToMapPercent(-43, -222),
    tileX: -43,
    tileY: -222,
    kind: 'port',
    regionId: 'new_aeven',
    blurb: 'Southern hub port — routes to Oakshore and the Desert.',
  },
  {
    id: 'desert_port',
    name: 'Desert Port',
    ...tileToMapPercent(-206, -240),
    tileX: -206,
    tileY: -240,
    kind: 'port',
    regionId: 'desert',
    blurb: 'Gateway to the sands — ships back to civilization or Oakshore.',
  },
  {
    id: 'south_obelisk',
    name: 'Southern Obelisk',
    ...tileToMapPercent(88, 34),
    tileX: 88,
    tileY: 34,
    kind: 'landmark',
    regionId: 'verdant',
    blurb: 'Waystone network — teleport north after the Obelisk Connection quest.',
  },
  {
    id: 'north_obelisk',
    name: 'Northern Obelisk',
    ...tileToMapPercent(92, -163),
    tileX: 92,
    tileY: -163,
    kind: 'landmark',
    regionId: 'north',
    blurb: 'Linked waystone — fast travel to the southern obelisk.',
  },
  {
    id: 'deep_desert',
    name: 'Deep Desert',
    ...tileToMapPercent(-410, -142),
    tileX: -410,
    tileY: -142,
    kind: 'travel',
    regionId: 'deep_desert',
    blurb: 'Camel route (1500g) — weapon shop, range shop, and bank.',
  },
  {
    id: 'desert_pass',
    name: 'Desert Pass',
    ...tileToMapPercent(-307, -212),
    tileX: -307,
    tileY: -212,
    kind: 'travel',
    regionId: 'desert',
    blurb: 'Camel crossing (1000g) through the dunes.',
  },
  {
    id: 'forbidden_cave',
    name: 'Forbidden Cave',
    ...tileToMapPercent(-235, -96),
    tileX: -235,
    tileY: -96,
    kind: 'dungeon',
    regionId: 'desert',
    blurb: 'Camel route (2000g) — remote cave content.',
  },
  {
    id: 'haunted_house',
    name: 'Haunted House',
    ...tileToMapPercent(24, 30),
    tileX: 24,
    tileY: 30,
    kind: 'dungeon',
    regionId: 'verdant',
    blurb: 'Interior dungeon — candle puzzles and the Ghastly Contraption quest.',
  },
  {
    id: 'pyramid_tomb',
    name: 'Pyramid Tomb',
    ...tileToMapPercent(-180, -180),
    tileX: -180,
    tileY: -180,
    kind: 'dungeon',
    regionId: 'desert',
    blurb: 'Pharaoh\'s Curse quest — ancient tomb interior.',
  },
  {
    id: 'koth_arena',
    name: 'King of the Hill Arena',
    ...tileToMapPercent(0, 0),
    tileX: 0,
    tileY: 0,
    kind: 'pvp',
    regionId: 'verdant',
    blurb: 'Instanced PvP arena interior.',
  },
  {
    id: 'wilderness_cabin',
    name: 'Wilderness Cabin',
    ...tileToMapPercent(64, -160),
    tileX: 64,
    tileY: -160,
    kind: 'pvp',
    regionId: 'wilderness',
    blurb: 'Wilderness PvP zone — other players can attack you here.',
  },
  {
    id: 'farming_patches',
    name: 'Allotment Plots',
    ...tileToMapPercent(-5, -26),
    tileX: -5,
    tileY: -26,
    kind: 'resource',
    regionId: 'oakshore',
    blurb: 'Player farming patches — grow crops for Cooking and Farming XP.',
  },
];

export const hubMarkers: HubMarker[] = [
  {
    id: 'new_aeven_bank',
    name: 'New Aeven Bank',
    ...tileToMapPercent(-55, -210),
    tileX: -55,
    tileY: -210,
    kind: 'bank',
    regionId: 'new_aeven',
  },
  {
    id: 'oakshore_bank',
    name: 'Oakshore Bank',
    ...tileToMapPercent(20, 40),
    tileX: 20,
    tileY: 40,
    kind: 'bank',
    regionId: 'oakshore',
  },
  {
    id: 'desert_bank',
    name: 'Desert Bank',
    ...tileToMapPercent(-195, -225),
    tileX: -195,
    tileY: -225,
    kind: 'bank',
    regionId: 'desert',
  },
  {
    id: 'new_aeven_smith',
    name: 'Blacksmith',
    ...tileToMapPercent(-48, -205),
    tileX: -48,
    tileY: -205,
    kind: 'shop',
    regionId: 'new_aeven',
  },
  {
    id: 'oakshore_magic',
    name: 'Magic Shop',
    ...tileToMapPercent(30, 48),
    tileX: 30,
    tileY: 48,
    kind: 'shop',
    regionId: 'oakshore',
  },
  {
    id: 'desert_jeweler',
    name: 'Jewel Shop',
    ...tileToMapPercent(-190, -215),
    tileX: -190,
    tileY: -215,
    kind: 'shop',
    regionId: 'desert',
  },
];

export const worldActivities: WorldActivity[] = [
  {
    id: 'gather',
    title: 'Gather',
    icon: '🪓',
    summary: 'Pull raw materials from the living world.',
    details: [
      'Woodcutting — oak, willow, maple, and yew trees scattered across the map (higher tiers need higher levels).',
      'Fishing — ponds (Lv.1), rivers (Lv.15), and ocean (Lv.40) with different loot tables.',
      'Herb gathering — forest herbs (Lv.1) and swamp herbs (Lv.15) via the Farming skill.',
      'Dig sites — shovel spots hidden around the world for buried loot.',
    ],
  },
  {
    id: 'build',
    title: 'Build',
    icon: '🏠',
    summary: 'Claim space and make the world yours.',
    details: [
      'Player housing interior — decorate and store items in your own house instance.',
      'Farming allotments — plant and harvest crops on dedicated plots near Oakshore.',
      'Persistent structures and land claims are planned as Solstead grows.',
    ],
  },
  {
    id: 'craft',
    title: 'Craft',
    icon: '⚒',
    summary: 'Turn materials into gear, food, and tools.',
    details: [
      'Blacksmith shops in New Aeven and the wild — smelt bars and forge weapons & armour.',
      'Tailors — craft ranged gear and robes.',
      'Cooking — use farmed ingredients at kitchens like Oakshore Cooker.',
      'Altars and crafting stations marked on your in-game world map.',
    ],
  },
  {
    id: 'trade',
    title: 'Trade',
    icon: '🪙',
    summary: 'Player-driven economy with NPC shops and banks.',
    details: [
      'Banks in every major hub — store gold and items safely (not in wilderness!).',
      'Specialty shops: magic, jewels, weapons, range gear, desert tents.',
      'Trade with other players face-to-face in towns and ports.',
    ],
  },
  {
    id: 'combat',
    title: 'Fight & Quest',
    icon: '⚔',
    summary: 'Monsters, dungeons, slayer, and storylines.',
    details: [
      'Overworld monsters — level up combat skills across regions.',
      'Slayer masters — task-based hunting in Oakshore and Desert caves.',
      'Dungeons — Haunted House, Reaper Dungeons, Spider Zone, Boss Caves, Pyramid Tomb.',
      'Quest lines — Tutorial, Awakening, Swamp, Desert Pharaoh, Cursed Lands, and more.',
    ],
  },
  {
    id: 'social',
    title: 'Socialize',
    icon: '💬',
    summary: 'Play together in a persistent online world.',
    details: [
      'See everyone in shared overworld instances — chat, emote, and party up.',
      'Duel arena and KOTH for structured PvP.',
      'Wilderness for high-stakes player combat.',
      'Ports and town hubs are natural meeting points.',
    ],
  },
];

export const interiorHighlights = [
  { name: 'New Aeven Bank', note: 'Main economic hub — deposit gear and gold.' },
  { name: 'Oakshore Farmhouse & Cooker', note: 'Farming loop and cooking training.' },
  { name: 'Desert Bank & Jewel Shop', note: 'Desert economy center.' },
  { name: 'Deep Desert Bank', note: 'Remote banking for far-west runs.' },
  { name: 'Haunted House', note: 'Multi-room puzzle dungeon.' },
  { name: 'Pyramid Tomb', note: 'Pharaoh boss content.' },
  { name: 'Underground Spider Zone', note: 'Swamp combat dungeon.' },
  { name: 'Reaper Dungeons', note: 'Northern death-themed instances.' },
  { name: 'Ye Old Pub', note: 'Social hangout interior.' },
  { name: 'KOTH / Duel Arena', note: 'Instanced PvP.' },
];

export const worldStats = {
  regions: worldRegions.length,
  pois: mapPois.length,
  dungeons: mapPois.filter((p) => p.kind === 'dungeon').length + 6,
  towns: mapPois.filter((p) => p.kind === 'port' || p.kind === 'town').length + 4,
  ports: mapPois.filter((p) => p.kind === 'port').length,
  interiors: interiorHighlights.length,
};
