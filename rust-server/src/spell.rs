//! Spell definitions and casting logic

use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellType {
    Damage,
    Heal,
    Teleport,
}

#[derive(Debug, Clone)]
pub struct SpellDef {
    pub id: &'static str,
    pub name: &'static str,
    pub spell_type: SpellType,
    pub magic_level_req: i32,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub base_power: i32,
    pub effect_sprite: &'static str,
}

pub const SPELLS: &[SpellDef] = &[
    SpellDef {
        id: "dark_hand",
        name: "Dark Hand",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 3,
        cooldown_ms: 1500,
        base_power: 3,
        effect_sprite: "dark_hand",
    },
    SpellDef {
        id: "lightning_bolt",
        name: "Lightning Bolt",
        spell_type: SpellType::Damage,
        magic_level_req: 7,
        mana_cost: 7,
        cooldown_ms: 2000,
        base_power: 5,
        effect_sprite: "lightning_bolt",
    },
    SpellDef {
        id: "dark_eater",
        name: "Dark Eater",
        spell_type: SpellType::Damage,
        magic_level_req: 15,
        mana_cost: 15,
        cooldown_ms: 3000,
        base_power: 8,
        effect_sprite: "dark_eater",
    },
    SpellDef {
        id: "rock_fall",
        name: "Rock Fall",
        spell_type: SpellType::Damage,
        magic_level_req: 25,
        mana_cost: 12,
        cooldown_ms: 2500,
        base_power: 10,
        effect_sprite: "rock_fall",
    },
    SpellDef {
        id: "heal",
        name: "Heal",
        spell_type: SpellType::Heal,
        magic_level_req: 5,
        mana_cost: 10,
        cooldown_ms: 5000,
        base_power: 5,
        effect_sprite: "self_heal",
    },
    SpellDef {
        id: "return_home",
        name: "Return Home",
        spell_type: SpellType::Teleport,
        magic_level_req: 0,
        mana_cost: 0,
        cooldown_ms: 900_000,
        base_power: 0,
        effect_sprite: "teleport",
    },
];

pub fn get_spell(id: &str) -> Option<&'static SpellDef> {
    SPELLS.iter().find(|s| s.id == id)
}

/// Calculate spell max hit: base_power + magic_level / 4
pub fn calculate_spell_max_hit(magic_level: i32, base_power: i32) -> i32 {
    base_power + magic_level / 4
}

/// Calculate heal amount: base_power + magic_level / 4
pub fn calculate_heal_amount(magic_level: i32, base_power: i32) -> i32 {
    base_power + magic_level / 4
}

/// Roll spell damage between 1 and max_hit (guaranteed minimum 1 on hit)
pub fn roll_spell_damage(max_hit: i32) -> i32 {
    if max_hit <= 0 {
        return 0;
    }
    rand::thread_rng().gen_range(1..=max_hit)
}
