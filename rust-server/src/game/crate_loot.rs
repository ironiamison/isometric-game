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
    pub fn load(data_path: &str) -> Result<Self, String> {
        let dir = format!("{}/crate_loot", data_path);
        let mut tables = HashMap::new();

        let entries = std::fs::read_dir(&dir)
            .map_err(|error| format!("failed to read crate_loot directory '{dir}': {error}"))?;

        for entry in entries {
            let path = entry
                .map_err(|error| format!("failed to read crate loot directory entry: {error}"))?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("invalid crate loot filename {}", path.display()))?
                .to_string();
            let source = std::fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let loot_file: CrateLootFile = toml::from_str(&source)
                .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
            validate_loot_table(&stem, &loot_file)?;
            tracing::info!("Loaded crate loot table: {}", stem);
            if tables.insert(stem.clone(), loot_file).is_some() {
                return Err(format!("duplicate crate loot table '{stem}'"));
            }
        }

        if tables.is_empty() {
            return Err("crate loot registry is empty".to_string());
        }
        tracing::info!("Loaded {} crate loot tables", tables.len());
        Ok(Self { tables })
    }

    pub fn validate_items(&self, items: &crate::data::ItemRegistry) -> Result<(), String> {
        for (table_id, table) in &self.tables {
            for entry in loot_entries(table) {
                if entry.item_id != "commission_marks" && items.get(&entry.item_id).is_none() {
                    return Err(format!(
                        "crate loot table '{table_id}' references unknown item '{}'",
                        entry.item_id
                    ));
                }
            }
        }
        Ok(())
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

fn validate_loot_table(id: &str, table: &CrateLootFile) -> Result<(), String> {
    let weights = &table.rarity_weights;
    if weights.common + weights.uncommon + weights.rare + weights.epic == 0 {
        return Err(format!(
            "crate loot table '{id}' has zero total rarity weight"
        ));
    }
    for entry in loot_entries(table) {
        if entry.item_id.is_empty()
            || entry.quantity_min <= 0
            || entry.quantity_max < entry.quantity_min
        {
            return Err(format!(
                "crate loot table '{id}' has invalid item '{}'",
                entry.item_id
            ));
        }
    }
    Ok(())
}

fn loot_entries(table: &CrateLootFile) -> impl Iterator<Item = &LootEntry> {
    [
        &table.low.common,
        &table.low.uncommon,
        &table.low.rare,
        &table.low.epic,
        &table.mid.common,
        &table.mid.uncommon,
        &table.mid.rare,
        &table.mid.epic,
        &table.high.common,
        &table.high.uncommon,
        &table.high.rare,
        &table.high.epic,
    ]
    .into_iter()
    .flatten()
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

        // 3. Write lock: all-or-nothing. Only consume the crate if the reward
        //    actually fits, so a full inventory can never eat the loot. We
        //    simulate on a clone because removing the crate frees its own slot
        //    first — the reward may fit only after that removal.
        let inventory_update = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) => p,
                None => return,
            };

            let mut trial = player.inventory.clone();
            trial.remove_item(&item_id, 1);

            if !is_commission_marks && !trial.try_add_item(&reward_id, qty, &self.item_registry) {
                None
            } else {
                player.inventory = trial;
                let slots = player.inventory.to_update();
                let gold = player.inventory.gold;
                Some((slots, gold))
            }
        };

        let inventory_update = match inventory_update {
            Some(update) => update,
            None => {
                self.send_system_message(
                    player_id,
                    "Your inventory is too full to open that crate — make some space and try again.",
                )
                .await;
                return;
            }
        };

        // Handle commission marks via database (outside write lock)
        if is_commission_marks
            && let Some(db) = &self.db
            && let Some(character_id) = Self::parse_character_id(player_id)
            && let Err(e) = db.add_commission_marks(character_id, qty).await
        {
            tracing::error!("Failed to add commission marks for {}: {}", player_id, e);
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
