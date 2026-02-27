//! Prayer System
//!
//! Provides prayer definitions and a registry for the Prayer skill.
//! Prayers provide temporary buffs at the cost of draining prayer points.
//!
//! Categories:
//! - Attack: Boosts attack accuracy
//! - Strength: Boosts max hit damage
//! - Defence: Boosts defence against attacks
//! - Protection: Reduces damage taken
//! - HPRegen: Increases HP regeneration rate
//! - Gathering: Boosts gathering speed

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

// ============================================================================
// Prayer Category
// ============================================================================

/// Prayer categories - only one prayer per category can be active at a time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrayerCategory {
    Attack,
    Strength,
    Defence,
    Protection,
    #[serde(rename = "hp_regen")]
    HPRegen,
    Gathering,
}

impl PrayerCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrayerCategory::Attack => "attack",
            PrayerCategory::Strength => "strength",
            PrayerCategory::Defence => "defence",
            PrayerCategory::Protection => "protection",
            PrayerCategory::HPRegen => "hp_regen",
            PrayerCategory::Gathering => "gathering",
        }
    }

    /// Get all prayer categories
    pub fn all() -> &'static [PrayerCategory] {
        &[
            PrayerCategory::Attack,
            PrayerCategory::Strength,
            PrayerCategory::Defence,
            PrayerCategory::Protection,
            PrayerCategory::HPRegen,
            PrayerCategory::Gathering,
        ]
    }
}

// ============================================================================
// Prayer Effect Types
// ============================================================================

/// Types of effects prayers can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrayerEffectType {
    /// Percentage bonus to attack accuracy
    AttackBonus,
    /// Percentage bonus to strength/max hit
    StrengthBonus,
    /// Percentage bonus to defence
    DefenceBonus,
    /// Percentage reduction in damage taken
    DamageReduction,
    /// Multiplier for HP regeneration rate
    #[serde(rename = "hp_regen_multiplier")]
    HPRegenMultiplier,
    /// Percentage bonus to gathering speed
    GatherSpeedBonus,
}

impl PrayerEffectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrayerEffectType::AttackBonus => "attack_bonus",
            PrayerEffectType::StrengthBonus => "strength_bonus",
            PrayerEffectType::DefenceBonus => "defence_bonus",
            PrayerEffectType::DamageReduction => "damage_reduction",
            PrayerEffectType::HPRegenMultiplier => "hp_regen_multiplier",
            PrayerEffectType::GatherSpeedBonus => "gather_speed_bonus",
        }
    }
}

// ============================================================================
// Raw TOML Structures
// ============================================================================

/// Raw prayer definition from TOML
#[derive(Debug, Clone, Deserialize)]
pub struct RawPrayerDefinition {
    pub name: Option<String>,
    pub description: Option<String>,
    pub level_req: i32,
    pub category: PrayerCategory,
    pub effect_type: PrayerEffectType,
    /// Effect value (percentage for bonuses, multiplier for HP regen)
    pub effect_value: f32,
    /// Drain rate in prayer points per tick (600ms)
    pub drain_rate: f32,
    /// Optional icon/sprite name for UI
    pub icon: Option<String>,
}

// ============================================================================
// Resolved Structures
// ============================================================================

/// A fully resolved prayer definition
#[derive(Debug, Clone)]
pub struct Prayer {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level_req: i32,
    pub category: PrayerCategory,
    pub effect_type: PrayerEffectType,
    pub effect_value: f32,
    pub drain_rate: f32,
    pub icon: String,
}

impl Prayer {
    /// Create a resolved Prayer from raw TOML data
    pub fn from_raw(id: &str, raw: &RawPrayerDefinition) -> Self {
        Self {
            id: id.to_string(),
            name: raw.name.clone().unwrap_or_else(|| id.replace('_', " ")),
            description: raw.description.clone().unwrap_or_default(),
            level_req: raw.level_req,
            category: raw.category,
            effect_type: raw.effect_type,
            effect_value: raw.effect_value,
            drain_rate: raw.drain_rate,
            icon: raw.icon.clone().unwrap_or_else(|| format!("prayer_{}", id)),
        }
    }

    /// Check if a player with the given prayer level can use this prayer
    pub fn can_use(&self, prayer_level: i32) -> bool {
        prayer_level >= self.level_req
    }
}

// ============================================================================
// Active Prayer Effects
// ============================================================================

/// Represents the combined effects of all active prayers
#[derive(Debug, Clone, Default)]
pub struct ActivePrayerEffects {
    /// Percentage attack bonus (e.g., 15.0 for +15%)
    pub attack_bonus: f32,
    /// Percentage strength bonus
    pub strength_bonus: f32,
    /// Percentage defence bonus
    pub defence_bonus: f32,
    /// Percentage damage reduction (e.g., 25.0 for -25% damage taken)
    pub damage_reduction: f32,
    /// HP regen multiplier (e.g., 2.0 for 2x regen)
    pub hp_regen_multiplier: f32,
    /// Percentage gathering speed bonus
    pub gather_speed_bonus: f32,
    /// Total drain rate per tick (sum of all active prayers)
    pub total_drain_rate: f32,
}

impl ActivePrayerEffects {
    /// Create new effects from a set of active prayers
    pub fn from_prayers<'a>(prayers: impl Iterator<Item = &'a Prayer>) -> Self {
        let mut effects = ActivePrayerEffects::default();
        effects.hp_regen_multiplier = 1.0; // Base multiplier

        for prayer in prayers {
            match prayer.effect_type {
                PrayerEffectType::AttackBonus => effects.attack_bonus += prayer.effect_value,
                PrayerEffectType::StrengthBonus => effects.strength_bonus += prayer.effect_value,
                PrayerEffectType::DefenceBonus => effects.defence_bonus += prayer.effect_value,
                PrayerEffectType::DamageReduction => {
                    effects.damage_reduction += prayer.effect_value
                }
                PrayerEffectType::HPRegenMultiplier => {
                    effects.hp_regen_multiplier = prayer.effect_value
                }
                PrayerEffectType::GatherSpeedBonus => {
                    effects.gather_speed_bonus += prayer.effect_value
                }
            }
            effects.total_drain_rate += prayer.drain_rate;
        }

        effects
    }

    /// Apply attack bonus to a base attack value
    pub fn apply_attack_bonus(&self, base_attack: i32) -> i32 {
        ((base_attack as f32) * (1.0 + self.attack_bonus / 100.0)).floor() as i32
    }

    /// Apply strength bonus to a base max hit
    pub fn apply_strength_bonus(&self, base_max_hit: i32) -> i32 {
        ((base_max_hit as f32) * (1.0 + self.strength_bonus / 100.0)).floor() as i32
    }

    /// Apply defence bonus to a base defence value
    pub fn apply_defence_bonus(&self, base_defence: i32) -> i32 {
        ((base_defence as f32) * (1.0 + self.defence_bonus / 100.0)).floor() as i32
    }

    /// Apply damage reduction to incoming damage
    pub fn apply_damage_reduction(&self, incoming_damage: i32) -> i32 {
        ((incoming_damage as f32) * (1.0 - self.damage_reduction / 100.0)).floor() as i32
    }

    /// Apply gathering speed bonus (returns speed multiplier)
    pub fn gather_speed_multiplier(&self) -> f32 {
        1.0 + self.gather_speed_bonus / 100.0
    }
}

// ============================================================================
// Prayer Registry
// ============================================================================

/// Registry for all prayer definitions
pub struct PrayerRegistry {
    prayers: HashMap<String, Prayer>,
    /// Prayers organized by category for quick lookup
    by_category: HashMap<PrayerCategory, Vec<String>>,
}

impl PrayerRegistry {
    pub fn new() -> Self {
        Self {
            prayers: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Load prayer definitions from a TOML file
    pub fn load_from_file(&mut self, file_path: &Path) -> Result<(), String> {
        if !file_path.exists() {
            warn!("Prayer file does not exist: {:?}", file_path);
            return Ok(());
        }

        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {:?}: {}", file_path, e))?;

        let table: HashMap<String, RawPrayerDefinition> = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse {:?}: {}", file_path, e))?;

        for (id, raw) in table {
            if self.prayers.contains_key(&id) {
                warn!("Duplicate prayer ID '{}', overwriting", id);
            }
            let prayer = Prayer::from_raw(&id, &raw);

            // Add to category index
            self.by_category
                .entry(prayer.category)
                .or_insert_with(Vec::new)
                .push(id.clone());

            self.prayers.insert(id, prayer);
        }

        info!("Loaded {} prayer definitions", self.prayers.len());

        Ok(())
    }

    /// Load prayer definitions from data directory
    pub fn load_from_directory(&mut self, data_dir: &Path) -> Result<(), String> {
        let prayers_file = data_dir.join("prayers.toml");
        self.load_from_file(&prayers_file)
    }

    /// Get a prayer by ID
    pub fn get(&self, id: &str) -> Option<&Prayer> {
        self.prayers.get(id)
    }

    /// Get a prayer by ID (alias for consistency with other registries)
    pub fn get_by_id(&self, id: &str) -> Option<&Prayer> {
        self.get(id)
    }

    /// Get all prayers in a category
    pub fn get_by_category(&self, category: PrayerCategory) -> Vec<&Prayer> {
        self.by_category
            .get(&category)
            .map(|ids| ids.iter().filter_map(|id| self.prayers.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all prayers available at a given prayer level
    pub fn available_at_level(&self, prayer_level: i32) -> Vec<&Prayer> {
        self.prayers
            .values()
            .filter(|p| p.level_req <= prayer_level)
            .collect()
    }

    /// Get all prayers available at a given level, organized by category
    pub fn available_by_category(
        &self,
        prayer_level: i32,
    ) -> HashMap<PrayerCategory, Vec<&Prayer>> {
        let mut result: HashMap<PrayerCategory, Vec<&Prayer>> = HashMap::new();

        for prayer in self.prayers.values() {
            if prayer.level_req <= prayer_level {
                result
                    .entry(prayer.category)
                    .or_insert_with(Vec::new)
                    .push(prayer);
            }
        }

        // Sort each category by level requirement
        for prayers in result.values_mut() {
            prayers.sort_by_key(|p| p.level_req);
        }

        result
    }

    /// Get all prayer IDs
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.prayers.keys()
    }

    /// Get all prayers
    pub fn all(&self) -> impl Iterator<Item = &Prayer> {
        self.prayers.values()
    }

    /// Check if a prayer exists
    pub fn contains(&self, id: &str) -> bool {
        self.prayers.contains_key(id)
    }

    /// Get the number of loaded prayers
    pub fn len(&self) -> usize {
        self.prayers.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.prayers.is_empty()
    }

    /// Calculate combined effects for a set of active prayer IDs
    pub fn calculate_effects(&self, active_prayer_ids: &[String]) -> ActivePrayerEffects {
        let prayers = active_prayer_ids
            .iter()
            .filter_map(|id| self.prayers.get(id));
        ActivePrayerEffects::from_prayers(prayers)
    }

    /// Validate that a set of prayers can be activated together
    /// (only one prayer per category allowed)
    pub fn validate_prayer_set(&self, prayer_ids: &[String]) -> Result<(), String> {
        let mut active_categories: HashMap<PrayerCategory, &str> = HashMap::new();

        for id in prayer_ids {
            if let Some(prayer) = self.prayers.get(id) {
                if let Some(existing) = active_categories.get(&prayer.category) {
                    return Err(format!(
                        "Cannot activate '{}' - '{}' is already active in the {} category",
                        id,
                        existing,
                        prayer.category.as_str()
                    ));
                }
                active_categories.insert(prayer.category, id);
            } else {
                return Err(format!("Unknown prayer: {}", id));
            }
        }

        Ok(())
    }
}

impl Default for PrayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_prayer_toml() -> String {
        r#"
[clarity]
name = "Clarity"
level_req = 1
category = "attack"
effect_type = "attack_bonus"
effect_value = 5.0
drain_rate = 1.0

[thick_skin]
name = "Thick Skin"
level_req = 1
category = "defence"
effect_type = "defence_bonus"
effect_value = 5.0
drain_rate = 1.0

[burst_of_strength]
name = "Burst of Strength"
level_req = 4
category = "strength"
effect_type = "strength_bonus"
effect_value = 5.0
drain_rate = 1.0

[protection]
name = "Protection"
level_req = 37
category = "protection"
effect_type = "damage_reduction"
effect_value = 25.0
drain_rate = 6.0

[rapid_heal]
name = "Rapid Heal"
level_req = 22
category = "hp_regen"
effect_type = "hp_regen_multiplier"
effect_value = 2.0
drain_rate = 2.0
"#
        .to_string()
    }

    #[test]
    fn test_load_prayers_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(create_test_prayer_toml().as_bytes())
            .unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();

        assert_eq!(registry.len(), 5);
        assert!(registry.contains("clarity"));
        assert!(registry.contains("thick_skin"));
        assert!(registry.contains("protection"));

        let clarity = registry.get("clarity").unwrap();
        assert_eq!(clarity.name, "Clarity");
        assert_eq!(clarity.level_req, 1);
        assert_eq!(clarity.category, PrayerCategory::Attack);
        assert_eq!(clarity.effect_type, PrayerEffectType::AttackBonus);
        assert_eq!(clarity.effect_value, 5.0);
        assert_eq!(clarity.drain_rate, 1.0);
    }

    #[test]
    fn test_get_by_category() {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(create_test_prayer_toml().as_bytes())
            .unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();

        let attack_prayers = registry.get_by_category(PrayerCategory::Attack);
        assert_eq!(attack_prayers.len(), 1);
        assert_eq!(attack_prayers[0].id, "clarity");

        let defence_prayers = registry.get_by_category(PrayerCategory::Defence);
        assert_eq!(defence_prayers.len(), 1);
        assert_eq!(defence_prayers[0].id, "thick_skin");
    }

    #[test]
    fn test_available_at_level() {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(create_test_prayer_toml().as_bytes())
            .unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();

        // Level 1: only clarity and thick_skin
        let level_1_prayers = registry.available_at_level(1);
        assert_eq!(level_1_prayers.len(), 2);

        // Level 10: clarity, thick_skin, burst_of_strength
        let level_10_prayers = registry.available_at_level(10);
        assert_eq!(level_10_prayers.len(), 3);

        // Level 37+: all 5 prayers
        let level_40_prayers = registry.available_at_level(40);
        assert_eq!(level_40_prayers.len(), 5);
    }

    #[test]
    fn test_calculate_effects() {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(create_test_prayer_toml().as_bytes())
            .unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();

        // Activate clarity (+5% attack) and burst_of_strength (+5% strength)
        let effects =
            registry.calculate_effects(&["clarity".to_string(), "burst_of_strength".to_string()]);

        assert_eq!(effects.attack_bonus, 5.0);
        assert_eq!(effects.strength_bonus, 5.0);
        assert_eq!(effects.total_drain_rate, 2.0); // 1.0 + 1.0
    }

    #[test]
    fn test_validate_prayer_set() {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");

        // Add two attack prayers for testing validation
        let toml_content = r#"
[clarity]
name = "Clarity"
level_req = 1
category = "attack"
effect_type = "attack_bonus"
effect_value = 5.0
drain_rate = 1.0

[improved_clarity]
name = "Improved Clarity"
level_req = 10
category = "attack"
effect_type = "attack_bonus"
effect_value = 10.0
drain_rate = 2.0

[thick_skin]
name = "Thick Skin"
level_req = 1
category = "defence"
effect_type = "defence_bonus"
effect_value = 5.0
drain_rate = 1.0
"#;

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(toml_content.as_bytes()).unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();

        // Valid: different categories
        assert!(
            registry
                .validate_prayer_set(&["clarity".to_string(), "thick_skin".to_string(),])
                .is_ok()
        );

        // Invalid: same category (attack)
        assert!(
            registry
                .validate_prayer_set(&["clarity".to_string(), "improved_clarity".to_string(),])
                .is_err()
        );
    }

    #[test]
    fn test_apply_effects() {
        let effects = ActivePrayerEffects {
            attack_bonus: 15.0,
            strength_bonus: 15.0,
            defence_bonus: 15.0,
            damage_reduction: 25.0,
            hp_regen_multiplier: 2.0,
            gather_speed_bonus: 20.0,
            total_drain_rate: 10.0,
        };

        // +15% attack: 100 -> 115
        assert_eq!(effects.apply_attack_bonus(100), 115);

        // +15% strength: 10 -> 11 (floor)
        assert_eq!(effects.apply_strength_bonus(10), 11);

        // +15% defence: 50 -> 57 (floor)
        assert_eq!(effects.apply_defence_bonus(50), 57);

        // -25% damage: 20 -> 15
        assert_eq!(effects.apply_damage_reduction(20), 15);

        // +20% gather speed: 1.2x multiplier
        assert!((effects.gather_speed_multiplier() - 1.2).abs() < 0.001);
    }

    #[test]
    fn test_prayer_can_use() {
        let prayer = Prayer {
            id: "test".to_string(),
            name: "Test Prayer".to_string(),
            description: "".to_string(),
            level_req: 10,
            category: PrayerCategory::Attack,
            effect_type: PrayerEffectType::AttackBonus,
            effect_value: 5.0,
            drain_rate: 1.0,
            icon: "test".to_string(),
        };

        assert!(!prayer.can_use(9));
        assert!(prayer.can_use(10));
        assert!(prayer.can_use(99));
    }

    #[test]
    fn test_load_real_prayers_toml() {
        // Test loading the actual prayers.toml from data directory
        let data_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/prayers.toml");

        let mut registry = PrayerRegistry::new();
        registry
            .load_from_file(&data_path)
            .expect("Failed to load prayers.toml");

        // Verify we have all 14 prayers
        assert_eq!(registry.len(), 14, "Expected 14 prayers in prayers.toml");

        // Verify specific prayers exist
        assert!(registry.contains("clarity"), "Missing clarity prayer");
        assert!(registry.contains("thick_skin"), "Missing thick_skin prayer");
        assert!(
            registry.contains("burst_of_strength"),
            "Missing burst_of_strength prayer"
        );
        assert!(
            registry.contains("improved_clarity"),
            "Missing improved_clarity prayer"
        );
        assert!(registry.contains("rock_skin"), "Missing rock_skin prayer");
        assert!(
            registry.contains("superhuman_strength"),
            "Missing superhuman_strength prayer"
        );
        assert!(
            registry.contains("resourcefulness"),
            "Missing resourcefulness prayer"
        );
        assert!(registry.contains("rapid_heal"), "Missing rapid_heal prayer");
        assert!(registry.contains("steel_skin"), "Missing steel_skin prayer");
        assert!(
            registry.contains("incredible_clarity"),
            "Missing incredible_clarity prayer"
        );
        assert!(
            registry.contains("ultimate_strength"),
            "Missing ultimate_strength prayer"
        );
        assert!(registry.contains("protection"), "Missing protection prayer");
        assert!(
            registry.contains("greater_resourcefulness"),
            "Missing greater_resourcefulness prayer"
        );
        assert!(
            registry.contains("greater_protection"),
            "Missing greater_protection prayer"
        );

        // Verify categories have correct prayers
        let attack_prayers = registry.get_by_category(PrayerCategory::Attack);
        assert_eq!(attack_prayers.len(), 3, "Expected 3 attack prayers");

        let defence_prayers = registry.get_by_category(PrayerCategory::Defence);
        assert_eq!(defence_prayers.len(), 3, "Expected 3 defence prayers");

        let strength_prayers = registry.get_by_category(PrayerCategory::Strength);
        assert_eq!(strength_prayers.len(), 3, "Expected 3 strength prayers");

        let protection_prayers = registry.get_by_category(PrayerCategory::Protection);
        assert_eq!(protection_prayers.len(), 2, "Expected 2 protection prayers");

        let hp_regen_prayers = registry.get_by_category(PrayerCategory::HPRegen);
        assert_eq!(hp_regen_prayers.len(), 1, "Expected 1 HP regen prayer");

        let gathering_prayers = registry.get_by_category(PrayerCategory::Gathering);
        assert_eq!(gathering_prayers.len(), 2, "Expected 2 gathering prayers");

        // Verify a specific prayer's properties
        let protection = registry.get("protection").unwrap();
        assert_eq!(protection.name, "Protection");
        assert_eq!(protection.level_req, 37);
        assert_eq!(protection.category, PrayerCategory::Protection);
        assert_eq!(protection.effect_type, PrayerEffectType::DamageReduction);
        assert_eq!(protection.effect_value, 25.0);
        assert_eq!(protection.drain_rate, 6.0);
    }
}
