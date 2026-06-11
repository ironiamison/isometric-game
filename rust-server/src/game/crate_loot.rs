use rand::Rng;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::collections::HashMap;

use super::GameRoom;
use crate::data::item_def::UseEffect;
use crate::protocol::ServerMessage;

// ============================================================================
// Data structures
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct LootEntry {
    pub item_id: String,
    pub quantity_min: i32,
    pub quantity_max: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RarityWeights {
    #[serde(default)]
    pub common: u32,
    #[serde(default)]
    pub uncommon: u32,
    #[serde(default)]
    pub rare: u32,
    #[serde(default)]
    pub epic: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BracketLoot {
    #[serde(default)]
    pub common: Vec<LootEntry>,
    #[serde(default)]
    pub uncommon: Vec<LootEntry>,
    #[serde(default)]
    pub rare: Vec<LootEntry>,
    #[serde(default)]
    pub epic: Vec<LootEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrateLootFile {
    pub rarity_weights: RarityWeights,
    #[serde(default)]
    pub low: BracketLoot,
    #[serde(default)]
    pub mid: BracketLoot,
    #[serde(default)]
    pub high: BracketLoot,
}

// ============================================================================
// Registry
// ============================================================================

pub struct CrateLootRegistry {
    tables: HashMap<String, CrateLootFile>,
}

impl CrateLootRegistry {
    pub fn load(data_path: &str) -> Self {
        let dir = format!("{}/crate_loot", data_path);
        let mut tables = HashMap::new();

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to read crate_loot directory '{}': {}", dir, e);
                return Self { tables };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<CrateLootFile>(&content) {
                    Ok(loot_file) => {
                        tracing::info!("Loaded crate loot table: {}", stem);
                        tables.insert(stem, loot_file);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse {}: {}", path.display(), e);
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read {}: {}", path.display(), e);
                }
            }
        }

        tracing::info!("Loaded {} crate loot tables", tables.len());
        Self { tables }
    }

    /// Roll a single loot reward from the given tier and bracket.
    /// Returns (item_id, quantity) or None if the tier/bracket is invalid.
    pub fn roll(&self, tier: &str, bracket: &str) -> Option<(String, i32)> {
        let table = self.tables.get(tier)?;
        let bracket_loot = match bracket {
            "low" => &table.low,
            "mid" => &table.mid,
            "high" => &table.high,
            _ => return None,
        };

        let weights = &table.rarity_weights;
        let total = weights.common + weights.uncommon + weights.rare + weights.epic;
        if total == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let roll = rng.gen_range(0..total);

        let pool = if roll < weights.common {
            &bracket_loot.common
        } else if roll < weights.common + weights.uncommon {
            &bracket_loot.uncommon
        } else if roll < weights.common + weights.uncommon + weights.rare {
            &bracket_loot.rare
        } else {
            &bracket_loot.epic
        };

        if pool.is_empty() {
            // Fallback to common if the rolled rarity pool is empty
            if bracket_loot.common.is_empty() {
                return None;
            }
            let entry = bracket_loot.common.choose(&mut rng)?;
            let qty = rng.gen_range(entry.quantity_min..=entry.quantity_max);
            return Some((entry.item_id.clone(), qty));
        }

        let entry = pool.choose(&mut rng)?;
        let qty = rng.gen_range(entry.quantity_min..=entry.quantity_max);
        Some((entry.item_id.clone(), qty))
    }
}

// ============================================================================
// Crate opening handler
// ============================================================================

impl GameRoom {
    pub(in crate::game) async fn handle_open_crate(&self, player_id: &str, slot_index: usize) {
        // 1. Read lock: get item_id, tier, bracket from the slot's UseEffect::OpenCrate
        let (item_id, tier, bracket) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };
            let slot = match player
                .inventory
                .slots
                .get(slot_index)
                .and_then(|s| s.as_ref())
            {
                Some(s) => s,
                None => return,
            };
            let def = match self.item_registry.get(&slot.item_id) {
                Some(d) => d,
                None => return,
            };
            match &def.use_effect {
                Some(UseEffect::OpenCrate { tier, bracket }) => {
                    (slot.item_id.clone(), tier.clone(), bracket.clone())
                }
                _ => return,
            }
        };

        // 2. Roll loot
        let (reward_id, qty) = match self.crate_loot_registry.roll(&tier, &bracket) {
            Some(r) => r,
            None => {
                self.send_system_message(player_id, "This crate has no loot configured.")
                    .await;
                return;
            }
        };

        let reward_display = self
            .item_registry
            .get(&reward_id)
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| reward_id.clone());

        let is_commission_marks = reward_id == "commission_marks";

        // 3. Write lock: remove crate, add reward (unless commission marks)
        let inventory_update = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            player.inventory.remove_item(&item_id, 1);

            if !is_commission_marks {
                player
                    .inventory
                    .add_item(&reward_id, qty, &self.item_registry);
            }

            let slots = player.inventory.to_update();
            let gold = player.inventory.gold;
            (slots, gold)
        };

        // Handle commission marks via database (outside write lock)
        if is_commission_marks {
            if let Some(db) = &self.db {
                if let Some(character_id) = Self::parse_character_id(player_id) {
                    if let Err(e) = db.add_commission_marks(character_id, qty).await {
                        tracing::error!("Failed to add commission marks for {}: {}", player_id, e);
                    }
                }
            }
        }

        // 4. Send InventoryUpdate
        let msg = ServerMessage::InventoryUpdate {
            player_id: player_id.to_string(),
            slots: inventory_update.0,
            gold: inventory_update.1,
        };
        self.send_to_player(player_id, msg).await;

        // 5. Send system message
        let feedback = format!("You open a crate and find: {}x {}!", qty, reward_display);
        self.send_system_message(player_id, &feedback).await;
    }
}
