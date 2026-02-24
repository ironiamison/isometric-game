//! Spell definitions for the client-side spell system

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellType {
    Damage,
    Heal,
    Teleport,
}

/// Static spell definition for UI display
#[derive(Clone, Debug)]
pub struct SpellDef {
    pub id: &'static str,
    pub name: &'static str,
    pub spell_type: SpellType,
    pub magic_level_req: i32,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub description: &'static str,
    pub effect_sprite: &'static str,
}

/// Scroll-exclusive spell definition (loaded from server at runtime)
#[derive(Clone, Debug)]
pub struct ScrollSpellDef {
    pub id: String,
    pub name: String,
    pub spell_type: SpellType,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub base_power: i32,
    pub effect_sprite: String,
    pub pushback_distance: i32,
    pub wall_slam_damage_per_tile: i32,
    pub description: String,
}

/// All spells in display order
pub const SPELLS: [SpellDef; 6] = [
    SpellDef {
        id: "dark_hand",
        name: "Dark Hand",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 3,
        cooldown_ms: 1500,
        description: "A shadowy hand strikes your target",
        effect_sprite: "dark_hand",
    },
    SpellDef {
        id: "lightning_bolt",
        name: "Lightning Bolt",
        spell_type: SpellType::Damage,
        magic_level_req: 7,
        mana_cost: 7,
        cooldown_ms: 2000,
        description: "A bolt of lightning strikes your target",
        effect_sprite: "lightning_bolt",
    },
    SpellDef {
        id: "dark_eater",
        name: "Dark Eater",
        spell_type: SpellType::Damage,
        magic_level_req: 15,
        mana_cost: 15,
        cooldown_ms: 3000,
        description: "A dark entity devours your target",
        effect_sprite: "dark_eater",
    },
    SpellDef {
        id: "rock_fall",
        name: "Rock Fall",
        spell_type: SpellType::Damage,
        magic_level_req: 25,
        mana_cost: 12,
        cooldown_ms: 2500,
        description: "Summon falling rocks to crush your target",
        effect_sprite: "rock_fall",
    },
    SpellDef {
        id: "heal",
        name: "Heal",
        spell_type: SpellType::Heal,
        magic_level_req: 5,
        mana_cost: 10,
        cooldown_ms: 5000,
        description: "Restore your health",
        effect_sprite: "self_heal",
    },
    SpellDef {
        id: "return_home",
        name: "Return Home",
        spell_type: SpellType::Teleport,
        magic_level_req: 0,
        mana_cost: 0,
        cooldown_ms: 900_000,
        description: "Teleport to the village spawn point",
        effect_sprite: "teleport",
    },
];
