//! Scroll-exclusive spell definitions loaded from TOML.
//! These spells can only be learned by using scroll items and are not gated by magic level.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct RawScrollSpellDef {
    pub name: String,
    pub spell_type: String,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub base_power: i32,
    pub effect_sprite: String,
    #[serde(default)]
    pub pushback_distance: i32,
    #[serde(default)]
    pub wall_slam_damage_per_tile: i32,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ScrollSpellDef {
    pub id: String,
    pub name: String,
    pub spell_type: crate::spell::SpellType,
    pub mana_cost: i32,
    pub cooldown_ms: u64,
    pub base_power: i32,
    pub effect_sprite: String,
    pub pushback_distance: i32,
    pub wall_slam_damage_per_tile: i32,
    pub description: String,
}

pub struct ScrollSpellRegistry {
    spells: HashMap<String, ScrollSpellDef>,
}

impl ScrollSpellRegistry {
    pub fn new() -> Self {
        Self {
            spells: HashMap::new(),
        }
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let raw: HashMap<String, RawScrollSpellDef> = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

        for (id, raw_def) in raw {
            let spell_type = match raw_def.spell_type.as_str() {
                "damage" => crate::spell::SpellType::Damage,
                "heal" => crate::spell::SpellType::Heal,
                "teleport" => crate::spell::SpellType::Teleport,
                other => {
                    return Err(format!(
                        "Unknown spell_type '{}' for scroll spell '{}'",
                        other, id
                    ));
                }
            };

            self.spells.insert(
                id.clone(),
                ScrollSpellDef {
                    id: id.clone(),
                    name: raw_def.name,
                    spell_type,
                    mana_cost: raw_def.mana_cost,
                    cooldown_ms: raw_def.cooldown_ms,
                    base_power: raw_def.base_power,
                    effect_sprite: raw_def.effect_sprite,
                    pushback_distance: raw_def.pushback_distance,
                    wall_slam_damage_per_tile: raw_def.wall_slam_damage_per_tile,
                    description: raw_def.description,
                },
            );
        }

        tracing::info!("Loaded {} scroll spell definitions", self.spells.len());
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ScrollSpellDef> {
        self.spells.get(id)
    }

    pub fn all(&self) -> &HashMap<String, ScrollSpellDef> {
        &self.spells
    }
}
