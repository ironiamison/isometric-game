//! Spell definitions and casting logic

use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellType {
    Damage,
    Heal,
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
        mana_cost: 5,
        cooldown_ms: 1500,
        base_power: 3,
        effect_sprite: "dark_hand",
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
        id: "heal",
        name: "Heal",
        spell_type: SpellType::Heal,
        magic_level_req: 5,
        mana_cost: 10,
        cooldown_ms: 5000,
        base_power: 5,
        effect_sprite: "self_heal",
    },
];

pub fn get_spell(id: &str) -> Option<&'static SpellDef> {
    SPELLS.iter().find(|s| s.id == id)
}

/// Calculate spell max hit: base_power + magic_level / 5
pub fn calculate_spell_max_hit(magic_level: i32, base_power: i32) -> i32 {
    base_power + magic_level / 5
}

/// Calculate heal amount: base_power + magic_level / 4
pub fn calculate_heal_amount(magic_level: i32, base_power: i32) -> i32 {
    base_power + magic_level / 4
}

/// Roll spell damage between 0 and max_hit
pub fn roll_spell_damage(max_hit: i32) -> i32 {
    if max_hit <= 0 {
        return 0;
    }
    rand::thread_rng().gen_range(0..=max_hit)
}
