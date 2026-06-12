use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

use super::item_def::{ItemDefinition, RawItemDefinition, WeaponType};

/// Registry for all item definitions
pub struct ItemRegistry {
    items: HashMap<String, ItemDefinition>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Load all item definitions from a directory
    pub fn load_from_directory(&mut self, data_dir: &Path) -> Result<(), String> {
        let items_dir = data_dir.join("items");

        if !items_dir.exists() {
            warn!("Items directory does not exist: {:?}", items_dir);
            return Ok(());
        }

        let entries = std::fs::read_dir(&items_dir)
            .map_err(|e| format!("Failed to read items directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "toml") {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

                // Parse as table of items
                let table: HashMap<String, RawItemDefinition> = toml::from_str(&content)
                    .map_err(|e| format!("Failed to parse {:?}: {}", path, e))?;

                for (id, raw) in table {
                    if self.items.contains_key(&id) {
                        warn!("Duplicate item ID '{}' in {:?}, overwriting", id, path);
                    }
                    let item = ItemDefinition::from_raw(&id, &raw);
                    // info!("Loaded item: {} ({})", item.display_name, id);
                    self.items.insert(id, item);
                }
            }
        }

        info!("Loaded {} item definitions", self.items.len());

        Ok(())
    }

    /// Get an item definition by ID
    pub fn get(&self, id: &str) -> Option<&ItemDefinition> {
        self.items.get(id)
    }

    /// Get all item IDs
    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.items.keys()
    }

    /// Get all items
    pub fn all(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.items.values()
    }

    /// Check if an item exists
    pub fn contains(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    /// Get the number of loaded items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Generate item definitions message for client sync
    pub fn to_client_definitions(&self) -> crate::protocol::ServerMessage {
        use super::item_def::EquipmentSlot;
        use crate::protocol::ClientItemDef;

        let items: Vec<ClientItemDef> = self
            .items
            .values()
            .map(|item| {
                // Extract equipment stats if present and has valid slot
                let (
                    equipment_slot,
                    attack_level_required,
                    defence_level_required,
                    ranged_level_required,
                    woodcutting_level_required,
                    mining_level_required,
                    attack_bonus,
                    strength_bonus,
                    defence_bonus,
                    magic_bonus,
                    magic_level_required,
                    chop_speed_multiplier,
                    mine_speed_multiplier,
                ) = if let Some(ref equip) = item.equipment {
                    if equip.slot_type != EquipmentSlot::None {
                        (
                            Some(equip.slot_type.as_str().to_string()),
                            Some(equip.attack_level_required),
                            Some(equip.defence_level_required),
                            Some(equip.ranged_level_required),
                            Some(equip.woodcutting_level_required),
                            Some(equip.mining_level_required),
                            Some(equip.attack_bonus),
                            Some(equip.strength_bonus),
                            Some(equip.defence_bonus),
                            Some(equip.magic_bonus),
                            Some(equip.magic_level_required),
                            if equip.chop_speed_multiplier > 0.0 {
                                Some(equip.chop_speed_multiplier)
                            } else {
                                None
                            },
                            if equip.mine_speed_multiplier > 0.0 {
                                Some(equip.mine_speed_multiplier)
                            } else {
                                None
                            },
                        )
                    } else {
                        (
                            None, None, None, None, None, None, None, None, None, None, None, None,
                            None,
                        )
                    }
                } else {
                    (
                        None, None, None, None, None, None, None, None, None, None, None, None,
                        None,
                    )
                };

                ClientItemDef {
                    id: item.id.clone(),
                    display_name: item.display_name.clone(),
                    sprite: item.sprite.clone(),
                    category: format!("{:?}", item.category).to_lowercase(),
                    max_stack: item.max_stack,
                    description: item.description.clone(),
                    base_price: item.base_price,
                    sellable: item.sellable,
                    equipment_slot,
                    attack_level_required,
                    defence_level_required,
                    ranged_level_required,
                    woodcutting_level_required,
                    mining_level_required,
                    attack_bonus,
                    strength_bonus,
                    defence_bonus,
                    magic_bonus,
                    magic_level_required,
                    weapon_type: item.equipment.as_ref().map(|e| match e.weapon_type {
                        WeaponType::Melee => "melee".to_string(),
                        WeaponType::Ranged => "ranged".to_string(),
                    }),
                    range: item.equipment.as_ref().map(|e| e.range),
                    chop_speed_multiplier,
                    mine_speed_multiplier,
                    prayer_xp: item.prayer_xp,
                    ranged_strength: item.ranged_strength,
                    ranged_strength_bonus: item.equipment.as_ref().and_then(|e| {
                        if e.ranged_strength_bonus > 0 {
                            Some(e.ranged_strength_bonus)
                        } else {
                            None
                        }
                    }),
                    use_effect_type: item.use_effect.as_ref().map(|e| match e {
                        super::UseEffect::Heal { .. } => "heal".to_string(),
                        super::UseEffect::RestoreMana { .. } => "restore_mana".to_string(),
                        super::UseEffect::RestorePrayer { .. } => "restore_prayer".to_string(),
                        super::UseEffect::Buff { .. } => "buff".to_string(),
                        super::UseEffect::Teleport { .. } => "teleport".to_string(),
                        super::UseEffect::LearnSpell { .. } => "learn_spell".to_string(),
                        super::UseEffect::Dig => "dig".to_string(),
                        super::UseEffect::OpenCrate { .. } => "open_crate".to_string(),
                    }),
                }
            })
            .collect();

        crate::protocol::ServerMessage::ItemDefinitions { items }
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_split_equipment_catalogs() {
        let mut registry = ItemRegistry::new();
        registry
            .load_from_directory(Path::new("data"))
            .expect("production item catalogs should parse");

        assert!(registry.contains("training_boots"));
        assert!(registry.contains("used_belt"));
        assert!(registry.contains("bronze_scimitar"));
    }
}
