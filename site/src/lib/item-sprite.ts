/** URL for an inventory item sprite PNG served from the game client assets. */
export function itemSpriteUrl(sprite: string): string {
  return `/play/assets/sprites/inventory/${sprite}.png`;
}

/** Map item sprites that are missing PNGs to the closest available art. */
const ITEM_SPRITE_ALIASES: Record<string, string> = {
  adamant_sword: 'steel_sword',
  ancient_fragment: 'bone_fragment',
  artisan_cape: 'wizard_robes',
  basement_key: 'gold_ring',
  construct_core: 'bone_fragment',
  dampening_crystal: 'gold_ring',
  dark_essence: 'bone_fragment',
  dragon_bones: 'bone_fragment',
  haunted_ectoplasm: 'bone_fragment',
  item_gold: 'gold_ring',
  mage_robe_male: 'wizard_robes',
  mage_robe_female: 'wizard_robes_female',
  magic_broom: 'wizard_robes',
  mithril_sword: 'steel_sword',
  necklace_vitality: 'gold_ring',
  pharaohs_key: 'gold_ring',
  raw_salmon: 'swordfish',
  raw_swordfish: 'swordfish',
  recipe_scroll: 'bone_fragment',
  refined_quartz: 'gold_ring',
  resonance_lens: 'gold_ring',
  reaper_scythe_fragment: 'bone_fragment',
  rune_sword: 'ancient_sword',
  spectral_coil: 'bone_fragment',
  spine_blade: 'ancient_sword',
  tinderbox: 'gold_ring',
  watermelon: 'swordfish',
  worn_pitchfork: 'iron_sword',
};

export function resolveItemSprite(sprite: string): string {
  return ITEM_SPRITE_ALIASES[sprite] ?? sprite;
}

/** Alternate sprite keys when the primary enemy PNG is missing from the asset pack. */
const ENTITY_SPRITE_ALIASES: Record<string, string> = {
  demon: 'reaper',
  animated_construct: 'rock_golem',
  seal_wraith: 'reaper',
  sand_wraith: 'reaper',
  ghost: 'reaper',
  pharaoh_mummy: 'skeleton',
  pharaoh_skeleton: 'skeleton',
  khareth_pharaoh: 'golden_scarab',
};

export function resolveEntitySprite(sprite: string): string {
  return ENTITY_SPRITE_ALIASES[sprite] ?? sprite;
}

/** URL for a bestiary creature sprite PNG. */
export function entitySpriteUrl(sprite: string): string {
  return `/play/assets/sprites/enemies/${sprite}.png`;
}
