//! Skills system following RuneScape-style mechanics.
//!
//! Skills: Hitpoints, Attack, Strength, Defence, Ranged, Fishing, Farming, Smithing, Prayer, Magic, Alchemy, Mining, Slayer
//! - Hitpoints: Max HP (1 HP per level, starts at 10)
//! - Attack: Accuracy in melee combat (starts at 1)
//! - Strength: Max hit in melee combat (starts at 1)
//! - Defence: Evasion rolls in combat (starts at 1)
//! - Ranged: Accuracy and damage with bows (starts at 1)
//! - Fishing: Gathering skill for catching fish
//! - Smithing: Crafting skill for forging weapons and armor
//! - Prayer: Max prayer points (1 point per level, starts at 1)
//! - Magic: Spell casting skill for damage and healing spells
//! - Mining: Gathering skill for mining ore from rocks

use rand::Rng;
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
            "hitpoints" | "hp" => Some(SkillType::Hitpoints),
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
/// Level 1 = 0 XP, Level 2 = 83 XP, Level 99 = 13,034,431 XP
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

/// Calculate level from total XP (inverse of total_xp_for_level)
pub fn level_for_xp(xp: i64) -> i32 {
    // Binary search for efficiency
    let mut low = 1;
    let mut high = MAX_LEVEL;

    while low < high {
        let mid = (low + high + 1) / 2;
        if total_xp_for_level(mid) <= xp {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

/// Individual skill data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub level: i32,
    pub xp: i64,
}

impl Skill {
    /// Create a new skill at the given level with appropriate XP
    pub fn new(level: i32) -> Self {
        Self {
            level: level.clamp(1, MAX_LEVEL),
            xp: total_xp_for_level(level),
        }
    }

    /// Add XP to this skill, returning true if leveled up
    pub fn add_xp(&mut self, amount: i64) -> bool {
        if self.level >= MAX_LEVEL {
            return false;
        }

        self.xp += amount;
        let new_level = level_for_xp(self.xp).min(MAX_LEVEL);

        if new_level > self.level {
            self.level = new_level;
            true
        } else {
            false
        }
    }

    /// XP needed to reach next level
    pub fn xp_to_next_level(&self) -> i64 {
        if self.level >= MAX_LEVEL {
            return 0;
        }
        total_xp_for_level(self.level + 1) - self.xp
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
    /// New character = combat level 3 (same as legacy)
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

/// Legacy skills format with combined `combat` field (previous format).
/// Migrates by splitting combat XP equally into attack/strength/defence.
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyCombatSkills {
    pub hitpoints: Skill,
    pub combat: Skill,
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

impl LegacyCombatSkills {
    /// Convert from combined combat to split attack/strength/defence by dividing XP ÷ 3
    pub fn to_skills(self) -> Skills {
        let split_xp = self.combat.xp / 3;
        let split_level = level_for_xp(split_xp);
        let split_skill = Skill {
            level: split_level,
            xp: split_xp,
        };
        Skills {
            hitpoints: self.hitpoints,
            attack: split_skill.clone(),
            strength: split_skill.clone(),
            defence: split_skill,
            ranged: Skill::new(1),
            fishing: self.fishing,
            farming: self.farming,
            smithing: self.smithing,
            prayer: self.prayer,
            magic: self.magic,
            woodcutting: self.woodcutting,
            alchemy: self.alchemy,
            mining: self.mining,
            slayer: self.slayer,
            survivalist: self.survivalist,
        }
    }
}

/// Ancient legacy skills format (hitpoints + attack + strength + defence only).
/// Converts to new format directly.
#[derive(Debug, Clone, Deserialize)]
pub struct LegacySkills {
    pub hitpoints: Skill,
    pub attack: Skill,
    pub strength: Skill,
    pub defence: Skill,
}

impl LegacySkills {
    /// Convert ancient legacy format to new format
    pub fn to_skills(self) -> Skills {
        Skills {
            hitpoints: self.hitpoints,
            attack: self.attack,
            strength: self.strength,
            defence: self.defence,
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
}

/// Calculate whether an attack hits using attack roll vs defence roll.
/// Returns true if the attack hits.
///
/// Formula: Roll attacker's accuracy (0 to (attack_level + 20) * (attack_bonus + 20))
///          Roll defender's evasion  (0 to (defence_level + 20) * (defence_bonus + 20))
///          Hit if attack_roll > defence_roll
///
/// The +20 base on level dampens level gaps so low-level NPCs can still
/// land hits on higher-level players (and vice versa). Same-level fights
/// remain 50/50 when bonuses are equal.
pub fn calculate_hit(
    attacker_attack_level: i32,
    attack_bonus: i32,
    defender_defence_level: i32,
    defence_bonus: i32,
) -> bool {
    let mut rng = rand::thread_rng();

    let attack_max = (attacker_attack_level + 20) * (attack_bonus + 20);
    let defence_max = (defender_defence_level + 20) * (defence_bonus + 20);

    let attack_roll = rng.gen_range(0..=attack_max.max(1));
    let defence_roll = rng.gen_range(0..=defence_max.max(1));

    attack_roll > defence_roll
}

/// Calculate maximum hit based on strength level and equipment bonus.
///
/// Formula: 1 + (strength_level / 16) + (strength_bonus / 4)
/// This gives roughly:
/// - Level 1, no bonus: 1
/// - Level 25, no bonus: 2
/// - Level 25, +10 bonus: 5
/// - Level 50, +20 bonus: 9
/// - Level 70, +25 bonus: 11
pub fn calculate_max_hit(strength_level: i32, strength_bonus: i32) -> i32 {
    let base = 1.0 + (strength_level as f64 / 16.0);
    let bonus = strength_bonus as f64 / 4.0;
    ((base + bonus).floor() as i32).max(1)
}

/// Roll damage between 1 and max_hit (inclusive).
/// Minimum damage on a successful hit is always 1.
pub fn roll_damage(max_hit: i32) -> i32 {
    if max_hit <= 1 {
        return 1;
    }
    rand::thread_rng().gen_range(1..=max_hit)
}

/// XP awarded per damage dealt by combat style.
/// Focused styles (Accurate/Aggressive/Defensive): 4.0 XP per damage to one skill.
/// Controlled: 1.33 XP per damage to each of Attack, Strength, Defence.
/// Hitpoints always gets 1.33 XP per damage.
pub const ATTACK_XP_PER_DAMAGE: f64 = 4.0;
pub const STRENGTH_XP_PER_DAMAGE: f64 = 4.0;
pub const DEFENCE_XP_PER_DAMAGE: f64 = 4.0;
pub const CONTROLLED_XP_PER_DAMAGE: f64 = 1.33;
pub const HITPOINTS_XP_PER_DAMAGE: f64 = 1.33;

/// Ranged XP constants
pub const RANGED_XP_PER_DAMAGE: f64 = 4.0;
pub const LONGRANGE_RANGED_XP_PER_DAMAGE: f64 = 2.0;
pub const LONGRANGE_DEFENCE_XP_PER_DAMAGE: f64 = 2.0;

/// Magic XP constants
pub const MAGIC_XP_PER_DAMAGE: f64 = 4.0;
pub const MAGIC_XP_PER_HEAL: f64 = 2.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xp_table() {
        // Known RS XP values
        assert_eq!(total_xp_for_level(1), 0);
        assert_eq!(total_xp_for_level(2), 83);
        assert!(total_xp_for_level(10) > 1000);
        assert!(total_xp_for_level(99) > 13_000_000);
    }

    #[test]
    fn test_level_for_xp() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(82), 1);
        assert_eq!(level_for_xp(83), 2);
        assert_eq!(level_for_xp(100), 2);

        // Round-trip test
        for level in 1..=99 {
            let xp = total_xp_for_level(level);
            assert_eq!(level_for_xp(xp), level);
        }
    }

    #[test]
    fn test_skill_add_xp() {
        let mut skill = Skill::new(1);
        assert_eq!(skill.level, 1);
        assert_eq!(skill.xp, 0);

        // Add enough XP to level up
        let leveled = skill.add_xp(100);
        assert!(leveled);
        assert_eq!(skill.level, 2);
    }

    #[test]
    fn test_combat_level() {
        let skills = Skills::new();
        // New character: HP 10, Atk 1, Str 1, Def 1, Ranged 1, Prayer 1, Magic 1
        // base = (1 + 10 + floor(1/2)) / 4 = (1 + 10 + 0) / 4 = 2.75
        // melee = (1 + 1) * 0.325 = 0.65
        // ranged = 1 * 0.4875 = 0.4875
        // magic = 1 * 0.4875 = 0.4875
        // combat_level = floor(2.75 + 0.65) = floor(3.4) = 3
        assert_eq!(skills.combat_level(), 3);

        // Max melee stats
        let max_skills = Skills {
            hitpoints: Skill::new(99),
            attack: Skill::new(99),
            strength: Skill::new(99),
            defence: Skill::new(99),
            prayer: Skill::new(99),
            ..Skills::new()
        };
        // base = (99 + 99 + floor(99/2)) / 4 = (99 + 99 + 49) / 4 = 61.75
        // melee = (99 + 99) * 0.325 = 64.35
        // combat_level = floor(61.75 + 64.35) = floor(126.1) = 126
        assert_eq!(max_skills.combat_level(), 126);

        // Max ranged stats
        let ranged_skills = Skills {
            hitpoints: Skill::new(99),
            ranged: Skill::new(99),
            defence: Skill::new(99),
            prayer: Skill::new(99),
            ..Skills::new()
        };
        // base = (99 + 99 + 49) / 4 = 61.75
        // ranged = 99 * 0.4875 = 48.2625
        // combat_level = floor(61.75 + 48.2625) = floor(110.0125) = 110
        assert_eq!(ranged_skills.combat_level(), 110);
    }

    #[test]
    fn test_total_level() {
        let skills = Skills::new();
        // HP 10 + Atk 1 + Str 1 + Def 1 + Ranged 1 + Fishing 1 + Farming 1 + Smithing 1 + Prayer 1 + Magic 1 + Woodcutting 1 + Alchemy 1 + Mining 1 + Slayer 1 + Survivalist 1 = 24
        assert_eq!(skills.total_level(), 24);
    }

    #[test]
    fn test_max_hit() {
        // Level 1, no bonus
        assert_eq!(calculate_max_hit(1, 0), 1);

        // Formula is 1 + floor(strength_level / 16) + floor(strength_bonus / 4) after summing.
        // At level 50 with no bonus: floor(1 + 50/16) = 4.
        assert_eq!(calculate_max_hit(50, 0), 4);

        // At level 99 with no bonus: floor(1 + 99/16) = 7.
        assert_eq!(calculate_max_hit(99, 0), 7);

        // Level 99, +50 bonus
        assert!(calculate_max_hit(99, 50) > 15);
    }

    #[test]
    fn test_legacy_combat_migration() {
        // Test migration from combined combat format
        let legacy = LegacyCombatSkills {
            hitpoints: Skill::new(50),
            combat: Skill::new(90),
            fishing: Skill::new(30),
            farming: Skill::new(1),
            smithing: Skill::new(1),
            prayer: Skill::new(1),
            magic: Skill::new(1),
            woodcutting: Skill::new(1),
            alchemy: Skill::new(1),
            mining: Skill::new(1),
            slayer: Skill::new(1),
            survivalist: Skill::new(1),
        };

        let skills = legacy.to_skills();

        // Hitpoints preserved
        assert_eq!(skills.hitpoints.level, 50);
        // Fishing preserved
        assert_eq!(skills.fishing.level, 30);
        // Combat XP split ÷ 3
        let expected_xp = total_xp_for_level(90) / 3;
        assert_eq!(skills.attack.xp, expected_xp);
        assert_eq!(skills.strength.xp, expected_xp);
        assert_eq!(skills.defence.xp, expected_xp);
    }

    #[test]
    fn test_ancient_legacy_migration() {
        let legacy = LegacySkills {
            hitpoints: Skill::new(50),
            attack: Skill::new(40),
            strength: Skill::new(35),
            defence: Skill::new(45),
        };

        let skills = legacy.to_skills();

        // All skills preserved directly
        assert_eq!(skills.hitpoints.level, 50);
        assert_eq!(skills.attack.level, 40);
        assert_eq!(skills.strength.level, 35);
        assert_eq!(skills.defence.level, 45);
    }
}
