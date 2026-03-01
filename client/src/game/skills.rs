//! Skills system for the client.
//!
//! This is a simplified version that tracks skill data received from the server.
//! All calculations happen server-side.
//!
//! Skills: Hitpoints, Attack, Strength, Defence, Ranged, Fishing, Farming, Smithing, Prayer, Magic, Woodcutting, Alchemy, Mining, Slayer, Survivalist
//! - Hitpoints: Max HP (1 HP per level, starts at 10)
//! - Attack: Accuracy in melee combat (starts at 1)
//! - Strength: Max hit in melee combat (starts at 1)
//! - Defence: Evasion rolls in combat (starts at 1)
//! - Ranged: Accuracy and damage with bows (starts at 1)
//! - Fishing: Gathering skill for catching fish
//! - Smithing: Crafting skill for forging weapons and armor
//! - Prayer: Enables prayer abilities that provide combat buffs
//! - Magic: Spellcasting skill for magic attacks and utility spells
//! - Woodcutting: Gathering skill for chopping trees

use serde::{Deserialize, Serialize};

/// Maximum skill level
pub const MAX_LEVEL: i32 = 99;

/// Skill types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillType {
    Hitpoints,
    Attack,
    Strength,
    Defence,
    Ranged,
    Fishing,
    Farming,
    Smithing,
    Prayer,
    Magic,
    Woodcutting,
    Alchemy,
    Mining,
    Slayer,
    Survivalist,
}

impl SkillType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillType::Hitpoints => "hitpoints",
            SkillType::Attack => "attack",
            SkillType::Strength => "strength",
            SkillType::Defence => "defence",
            SkillType::Ranged => "ranged",
            SkillType::Fishing => "fishing",
            SkillType::Farming => "farming",
            SkillType::Smithing => "smithing",
            SkillType::Prayer => "prayer",
            SkillType::Magic => "magic",
            SkillType::Woodcutting => "woodcutting",
            SkillType::Alchemy => "alchemy",
            SkillType::Mining => "mining",
            SkillType::Slayer => "slayer",
            SkillType::Survivalist => "survivalist",
        }
    }

    pub fn from_str(s: &str) -> Option<SkillType> {
        match s.to_lowercase().as_str() {
            "hitpoints" => Some(SkillType::Hitpoints),
            "attack" => Some(SkillType::Attack),
            "strength" => Some(SkillType::Strength),
            "defence" => Some(SkillType::Defence),
            "ranged" => Some(SkillType::Ranged),
            "fishing" => Some(SkillType::Fishing),
            "farming" => Some(SkillType::Farming),
            "smithing" => Some(SkillType::Smithing),
            "prayer" => Some(SkillType::Prayer),
            "magic" => Some(SkillType::Magic),
            "woodcutting" => Some(SkillType::Woodcutting),
            "alchemy" => Some(SkillType::Alchemy),
            "mining" => Some(SkillType::Mining),
            "slayer" => Some(SkillType::Slayer),
            "survivalist" => Some(SkillType::Survivalist),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SkillType::Hitpoints => "Hitpoints",
            SkillType::Attack => "Attack",
            SkillType::Strength => "Strength",
            SkillType::Defence => "Defence",
            SkillType::Ranged => "Ranged",
            SkillType::Fishing => "Fishing",
            SkillType::Farming => "Farming",
            SkillType::Smithing => "Smithing",
            SkillType::Prayer => "Prayer",
            SkillType::Magic => "Magic",
            SkillType::Woodcutting => "Woodcutting",
            SkillType::Alchemy => "Alchemy",
            SkillType::Mining => "Mining",
            SkillType::Slayer => "Slayer",
            SkillType::Survivalist => "Survivalist",
        }
    }

    pub fn all() -> &'static [SkillType] {
        &[
            SkillType::Hitpoints,
            SkillType::Attack,
            SkillType::Strength,
            SkillType::Defence,
            SkillType::Ranged,
            SkillType::Fishing,
            SkillType::Farming,
            SkillType::Smithing,
            SkillType::Prayer,
            SkillType::Magic,
            SkillType::Woodcutting,
            SkillType::Alchemy,
            SkillType::Mining,
            SkillType::Slayer,
            SkillType::Survivalist,
        ]
    }
}

/// Calculate total XP required to reach a level using RuneScape formula.
/// Used for calculating XP progress within a level.
pub fn total_xp_for_level(level: i32) -> i64 {
    if level <= 1 {
        return 0;
    }
    let mut total = 0.0;
    for l in 1..level {
        total += (l as f64 + 300.0 * 2.0_f64.powf(l as f64 / 7.0)) / 4.0;
    }
    total.floor() as i64
}

/// Individual skill data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub level: i32,
    pub xp: i64,
}

impl Skill {
    pub fn new(level: i32) -> Self {
        Self {
            level: level.clamp(1, MAX_LEVEL),
            xp: total_xp_for_level(level),
        }
    }

    /// XP progress within current level (0.0 to 1.0)
    pub fn level_progress(&self) -> f32 {
        if self.level >= MAX_LEVEL {
            return 1.0;
        }
        let current_level_xp = total_xp_for_level(self.level);
        let next_level_xp = total_xp_for_level(self.level + 1);
        let xp_in_level = self.xp - current_level_xp;
        let xp_needed = next_level_xp - current_level_xp;
        (xp_in_level as f32 / xp_needed as f32).clamp(0.0, 1.0)
    }

    /// XP needed to reach next level
    pub fn xp_to_next_level(&self) -> i64 {
        if self.level >= MAX_LEVEL {
            return 0;
        }
        total_xp_for_level(self.level + 1) - self.xp
    }
}

impl Default for Skill {
    fn default() -> Self {
        Self::new(1)
    }
}

/// All skills for a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skills {
    pub hitpoints: Skill,
    #[serde(default)]
    pub attack: Skill,
    #[serde(default)]
    pub strength: Skill,
    #[serde(default)]
    pub defence: Skill,
    #[serde(default)]
    pub ranged: Skill,
    #[serde(default)]
    pub fishing: Skill,
    #[serde(default)]
    pub farming: Skill,
    #[serde(default)]
    pub smithing: Skill,
    #[serde(default)]
    pub prayer: Skill,
    #[serde(default)]
    pub magic: Skill,
    #[serde(default)]
    pub woodcutting: Skill,
    #[serde(default)]
    pub alchemy: Skill,
    #[serde(default)]
    pub mining: Skill,
    #[serde(default)]
    pub slayer: Skill,
    #[serde(default)]
    pub survivalist: Skill,
}

impl Default for Skills {
    fn default() -> Self {
        Self::new()
    }
}

impl Skills {
    /// Create new skills with starting values (HP 10, Attack/Strength/Defence/Ranged 1, others 1)
    pub fn new() -> Self {
        Self {
            hitpoints: Skill::new(10),
            attack: Skill::new(1),
            strength: Skill::new(1),
            defence: Skill::new(1),
            ranged: Skill::new(1),
            fishing: Skill::new(1),
            farming: Skill::new(1),
            smithing: Skill::new(1),
            prayer: Skill::new(1),
            magic: Skill::new(1),
            woodcutting: Skill::new(1),
            alchemy: Skill::new(1),
            mining: Skill::new(1),
            slayer: Skill::new(1),
            survivalist: Skill::new(1),
        }
    }

    /// Calculate combat level using OSRS-style formula:
    /// base = (Defence + Hitpoints + floor(Prayer/2)) / 4
    /// combat_level = floor(base + max((Attack+Strength)*0.325, Ranged*0.4875, Magic*0.4875))
    pub fn combat_level(&self) -> i32 {
        let base = (self.defence.level as f64 + self.hitpoints.level as f64
            + (self.prayer.level as f64 / 2.0).floor())
            / 4.0;
        let melee = (self.attack.level + self.strength.level) as f64 * 0.325;
        let ranged = self.ranged.level as f64 * 0.4875;
        let magic = self.magic.level as f64 * 0.4875;
        (base + melee.max(ranged).max(magic)).floor() as i32
    }

    /// Get a skill by type
    pub fn get(&self, skill_type: SkillType) -> &Skill {
        match skill_type {
            SkillType::Hitpoints => &self.hitpoints,
            SkillType::Attack => &self.attack,
            SkillType::Strength => &self.strength,
            SkillType::Defence => &self.defence,
            SkillType::Ranged => &self.ranged,
            SkillType::Fishing => &self.fishing,
            SkillType::Farming => &self.farming,
            SkillType::Smithing => &self.smithing,
            SkillType::Prayer => &self.prayer,
            SkillType::Magic => &self.magic,
            SkillType::Woodcutting => &self.woodcutting,
            SkillType::Alchemy => &self.alchemy,
            SkillType::Mining => &self.mining,
            SkillType::Slayer => &self.slayer,
            SkillType::Survivalist => &self.survivalist,
        }
    }

    /// Get a mutable skill by type
    pub fn get_mut(&mut self, skill_type: SkillType) -> &mut Skill {
        match skill_type {
            SkillType::Hitpoints => &mut self.hitpoints,
            SkillType::Attack => &mut self.attack,
            SkillType::Strength => &mut self.strength,
            SkillType::Defence => &mut self.defence,
            SkillType::Ranged => &mut self.ranged,
            SkillType::Fishing => &mut self.fishing,
            SkillType::Farming => &mut self.farming,
            SkillType::Smithing => &mut self.smithing,
            SkillType::Prayer => &mut self.prayer,
            SkillType::Magic => &mut self.magic,
            SkillType::Woodcutting => &mut self.woodcutting,
            SkillType::Alchemy => &mut self.alchemy,
            SkillType::Mining => &mut self.mining,
            SkillType::Slayer => &mut self.slayer,
            SkillType::Survivalist => &mut self.survivalist,
        }
    }

    /// Update a skill from server data
    pub fn update_skill(&mut self, skill_type: SkillType, xp: i64, level: i32) {
        let skill = self.get_mut(skill_type);
        skill.xp = xp;
        skill.level = level;
    }

    /// Total level (sum of all skill levels)
    pub fn total_level(&self) -> i32 {
        self.hitpoints.level
            + self.attack.level
            + self.strength.level
            + self.defence.level
            + self.ranged.level
            + self.fishing.level
            + self.farming.level
            + self.smithing.level
            + self.prayer.level
            + self.magic.level
            + self.woodcutting.level
            + self.alchemy.level
            + self.mining.level
            + self.slayer.level
            + self.survivalist.level
    }
}
