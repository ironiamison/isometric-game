//! Spell definitions for the client-side spell system

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellType {
    Damage,
    Heal,
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

/// All spells in display order
pub const SPELLS: [SpellDef; 3] = [
    SpellDef {
        id: "dark_hand",
        name: "Dark Hand",
        spell_type: SpellType::Damage,
        magic_level_req: 1,
        mana_cost: 5,
        cooldown_ms: 1500,
        description: "A shadowy hand strikes your target",
        effect_sprite: "dark_hand",
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
        id: "heal",
        name: "Heal",
        spell_type: SpellType::Heal,
        magic_level_req: 5,
        mana_cost: 10,
        cooldown_ms: 5000,
        description: "Restore your health",
        effect_sprite: "self_heal",
    },
];
