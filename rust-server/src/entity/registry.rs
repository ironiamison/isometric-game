use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::info;

use super::prototype::{
    AnimationType, EntityBehaviors, EntityPrototype, LootEntry, LootTable, RawEntityPrototype,
    ResolvedRewards, ResolvedStats, SpeechConfig,
};

/// Registry for all entity prototypes
pub struct EntityRegistry {
    prototypes: HashMap<String, EntityPrototype>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
        }
    }

    /// Load all entity definitions from a directory
    pub fn load_from_directory(&mut self, data_dir: &Path) -> Result<(), String> {
        let entities_dir = data_dir.join("entities");

        // First pass: load all raw prototypes
        let mut raw_prototypes: HashMap<String, RawEntityPrototype> = HashMap::new();

        // Load monsters
        let monsters_dir = entities_dir.join("monsters");
        if monsters_dir.exists() {
            self.load_toml_files(&monsters_dir, &mut raw_prototypes)?;
        }

        // Load NPCs
        let npcs_dir = entities_dir.join("npcs");
        if npcs_dir.exists() {
            self.load_toml_files(&npcs_dir, &mut raw_prototypes)?;
        }

        info!("Loaded {} raw entity prototypes", raw_prototypes.len());

        // Second pass: resolve inheritance
        self.resolve_all_prototypes(raw_prototypes)?;

        info!("Resolved {} entity prototypes", self.prototypes.len());

        Ok(())
    }

    fn load_toml_files(
        &self,
        dir: &Path,
        raw_prototypes: &mut HashMap<String, RawEntityPrototype>,
    ) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "toml") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

                // Parse as table of entities
                let table: HashMap<String, RawEntityPrototype> = toml::from_str(&content)
                    .map_err(|e| format!("Failed to parse {:?}: {}", path, e))?;

                for (id, proto) in table {
                    if raw_prototypes.contains_key(&id) {
                        return Err(format!("Duplicate entity ID '{}' in {:?}", id, path));
                    }
                    raw_prototypes.insert(id, proto);
                }
            }
        }

        Ok(())
    }

    fn resolve_all_prototypes(
        &mut self,
        raw_prototypes: HashMap<String, RawEntityPrototype>,
    ) -> Result<(), String> {
        // Topological sort to handle inheritance order
        let sorted_ids = self.topological_sort(&raw_prototypes)?;

        for id in sorted_ids {
            let raw = raw_prototypes.get(&id).unwrap();
            let resolved = self.resolve_prototype(&id, raw)?;
            self.prototypes.insert(id, resolved);
        }

        Ok(())
    }

    fn topological_sort(
        &self,
        raw_prototypes: &HashMap<String, RawEntityPrototype>,
    ) -> Result<Vec<String>, String> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        fn visit(
            id: &str,
            raw_prototypes: &HashMap<String, RawEntityPrototype>,
            sorted: &mut Vec<String>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
        ) -> Result<(), String> {
            if visited.contains(id) {
                return Ok(());
            }
            if visiting.contains(id) {
                return Err(format!("Circular inheritance detected at '{}'", id));
            }

            visiting.insert(id.to_string());

            if let Some(raw) = raw_prototypes.get(id)
                && let Some(parent_id) = &raw.extends
            {
                if !raw_prototypes.contains_key(parent_id) {
                    return Err(format!(
                        "Entity '{}' extends unknown parent '{}'",
                        id, parent_id
                    ));
                }
                visit(parent_id, raw_prototypes, sorted, visited, visiting)?;
            }

            visiting.remove(id);
            visited.insert(id.to_string());
            sorted.push(id.to_string());

            Ok(())
        }

        for id in raw_prototypes.keys() {
            visit(id, raw_prototypes, &mut sorted, &mut visited, &mut visiting)?;
        }

        Ok(sorted)
    }

    fn resolve_prototype(
        &self,
        id: &str,
        raw: &RawEntityPrototype,
    ) -> Result<EntityPrototype, String> {
        // Get parent prototype if extends is specified
        let parent = if let Some(parent_id) = &raw.extends {
            self.prototypes.get(parent_id)
        } else {
            None
        };

        // Merge stats with parent (child overrides parent)
        let stats = ResolvedStats {
            level: raw
                .stats
                .level
                .or_else(|| parent.map(|p| p.stats.level))
                .unwrap_or(1),
            max_hp: raw
                .stats
                .max_hp
                .or_else(|| parent.map(|p| p.stats.max_hp))
                .unwrap_or(100),
            damage: raw
                .stats
                .damage
                .or_else(|| parent.map(|p| p.stats.damage))
                .unwrap_or(10),
            attack_bonus: raw
                .stats
                .attack_bonus
                .or_else(|| parent.map(|p| p.stats.attack_bonus))
                .unwrap_or(0),
            defence_bonus: raw
                .stats
                .defence_bonus
                .or_else(|| parent.map(|p| p.stats.defence_bonus))
                .unwrap_or(0),
            attack_range: raw
                .stats
                .attack_range
                .or_else(|| parent.map(|p| p.stats.attack_range))
                .unwrap_or(1),
            aggro_range: raw
                .stats
                .aggro_range
                .or_else(|| parent.map(|p| p.stats.aggro_range))
                .unwrap_or(5),
            chase_range: raw
                .stats
                .chase_range
                .or_else(|| parent.map(|p| p.stats.chase_range))
                .unwrap_or(8),
            move_cooldown_ms: raw
                .stats
                .move_cooldown_ms
                .or_else(|| parent.map(|p| p.stats.move_cooldown_ms))
                .unwrap_or(500),
            attack_cooldown_ms: raw
                .stats
                .attack_cooldown_ms
                .or_else(|| parent.map(|p| p.stats.attack_cooldown_ms))
                .unwrap_or(800),
            respawn_time_ms: raw
                .stats
                .respawn_time_ms
                .or_else(|| parent.map(|p| p.stats.respawn_time_ms))
                .unwrap_or(10000),
            hp_regen_percent_per_sec: raw
                .stats
                .hp_regen_percent_per_sec
                .or_else(|| parent.map(|p| p.stats.hp_regen_percent_per_sec))
                .unwrap_or(2.0),
        };

        // Merge rewards
        let rewards = ResolvedRewards {
            exp_base: raw
                .rewards
                .exp_base
                .or_else(|| parent.map(|p| p.rewards.exp_base))
                .unwrap_or(10),
            gold_min: raw
                .rewards
                .gold_min
                .or_else(|| parent.map(|p| p.rewards.gold_min))
                .unwrap_or(1),
            gold_max: raw
                .rewards
                .gold_max
                .or_else(|| parent.map(|p| p.rewards.gold_max))
                .unwrap_or(5),
        };

        // Merge loot tables (child appends to parent)
        let mut loot: Vec<LootEntry> = parent.map(|p| p.loot.clone()).unwrap_or_default();
        loot.extend(raw.loot.clone());

        // Merge loot roll tables (child appends to parent)
        let mut loot_tables: Vec<LootTable> =
            parent.map(|p| p.loot_tables.clone()).unwrap_or_default();
        loot_tables.extend(raw.loot_tables.clone());

        let size = raw
            .size
            .unwrap_or_else(|| parent.map(|p| p.size).unwrap_or(1));

        // Parse animation type
        let animation_type = raw
            .animation_type
            .as_deref()
            .map(AnimationType::from_str)
            .or_else(|| parent.map(|p| p.animation_type))
            .unwrap_or_default();

        // Merge tags (child inherits parent tags + adds own)
        let mut tags: Vec<String> = parent.map(|p| p.tags.clone()).unwrap_or_default();
        for tag in &raw.tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }

        // Merge behaviors (child overrides if set)
        let behaviors = EntityBehaviors::from(&raw.behaviors);

        Ok(EntityPrototype {
            id: id.to_string(),
            display_name: raw
                .display_name
                .clone()
                .or_else(|| parent.map(|p| p.display_name.clone()))
                .unwrap_or_else(|| id.to_string()),
            sprite: raw
                .sprite
                .clone()
                .or_else(|| parent.map(|p| p.sprite.clone()))
                .unwrap_or_else(|| "unknown".to_string()),
            animation_type,
            description: raw
                .description
                .clone()
                .or_else(|| parent.map(|p| p.description.clone()))
                .unwrap_or_default(),
            tags,
            stats,
            rewards,
            loot,
            loot_tables,
            behaviors,
            merchant: raw
                .merchant
                .clone()
                .or_else(|| parent.and_then(|p| p.merchant.clone())),
            quest_giver: raw
                .quest_giver
                .clone()
                .or_else(|| parent.and_then(|p| p.quest_giver.clone())),
            dialogue: raw.dialogue.clone().unwrap_or_default(),
            speech: raw
                .speech
                .as_ref()
                .map(SpeechConfig::from)
                .or_else(|| parent.and_then(|p| p.speech.clone())),
            port: raw
                .port
                .clone()
                .or_else(|| parent.and_then(|p| p.port.clone())),
            size,
        })
    }

    /// Get a prototype by ID
    pub fn get(&self, id: &str) -> Option<&EntityPrototype> {
        self.prototypes.get(id)
    }

    /// Get all prototype IDs
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.prototypes.keys()
    }

    /// Get all prototypes
    pub fn all(&self) -> impl Iterator<Item = &EntityPrototype> {
        self.prototypes.values()
    }

    /// Check if a prototype exists
    pub fn contains(&self, id: &str) -> bool {
        self.prototypes.contains_key(id)
    }

    /// Get the number of loaded prototypes
    pub fn len(&self) -> usize {
        self.prototypes.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.prototypes.is_empty()
    }

    pub fn validate_items(&self, items: &crate::data::ItemRegistry) -> Result<(), String> {
        for prototype in self.prototypes.values() {
            for loot in &prototype.loot {
                if loot.item_id != "nothing" && items.get(&loot.item_id).is_none() {
                    return Err(format!(
                        "entity '{}' references unknown loot item '{}'",
                        prototype.id, loot.item_id
                    ));
                }
                if !(0.0..=1.0).contains(&loot.drop_chance)
                    || loot.quantity_min <= 0
                    || loot.quantity_max < loot.quantity_min
                {
                    return Err(format!(
                        "entity '{}' has invalid loot range for '{}'",
                        prototype.id, loot.item_id
                    ));
                }
            }
            for table in &prototype.loot_tables {
                if !(0.0..=1.0).contains(&table.chance) {
                    return Err(format!(
                        "entity '{}' has invalid loot table chance for '{}'",
                        prototype.id, table.name
                    ));
                }
                for entry in &table.entries {
                    if (entry.item_id != "nothing" && items.get(&entry.item_id).is_none())
                        || entry.weight <= 0
                        || entry.quantity_min <= 0
                        || entry.quantity_max < entry.quantity_min
                    {
                        return Err(format!(
                            "entity '{}' has invalid loot table item '{}'",
                            prototype.id, entry.item_id
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for EntityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_split_dangerous_creature_catalogs() {
        let mut registry = EntityRegistry::new();
        registry
            .load_from_directory(Path::new("data"))
            .expect("production entity catalogs should parse");

        assert!(registry.contains("spider"));
        assert!(registry.contains("barbarian"));
    }
}
