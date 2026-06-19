export type WikiBlock =
  | { type: 'p'; text: string }
  | { type: 'h2'; text: string }
  | { type: 'ul'; items: string[] }
  | { type: 'html'; html: string }
  | { type: 'link'; href: string; label: string };

export type WikiSection =
  | 'getting-started'
  | 'world'
  | 'gameplay'
  | 'content'
  | 'community';

export type WikiArticle = {
  slug: string;
  title: string;
  summary: string;
  section: WikiSection;
  icon?: string;
  thumbnail?: string;
  updatedAt: string;
  popular?: boolean;
  externalLink?: string;
  blocks: WikiBlock[];
};

export type WikiNavGroup = {
  id: WikiSection;
  label: string;
  links: { slug: string; label: string; icon?: string }[];
};

export type GameQuest = {
  id: string;
  slug: string;
  name: string;
  description: string;
  giver_npc: string;
  level_required: number;
  repeatable: boolean;
  folder: string;
  exp: number;
  gold: number;
};

export type WikiGameData = {
  generated_at: string;
  stats: {
    quests: number;
    items: number;
    entities: number;
    recipes: number;
    shops: number;
    interiors: number;
  };
  quests: GameQuest[];
  items: GameItem[];
  entities: GameEntity[];
  interiors: string[];
  shops: string[];
};

export type GameItem = {
  id: string;
  display_name: string;
  sprite: string;
  description: string;
  category: string;
  max_stack: number;
  base_price: number;
  sellable: boolean;
  equipment: GameItemEquipment | null;
};

export type GameItemEquipment = {
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
};

export type GameEntity = {
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
  loot: { item_id: string; drop_chance: number; quantity_min: number; quantity_max: number }[];
  loot_tables: {
    name: string;
    chance: number;
    entries: { item_id: string; weight: number; quantity_min: number; quantity_max: number }[];
  }[];
  quest_ids: string[];
};
