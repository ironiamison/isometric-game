use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;
use tracing::{info, warn};

use crate::data::ItemRegistry;
use crate::item::InventorySlot;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of slots a chest can have
pub const MAX_CHEST_SLOTS: usize = 10;

// ============================================================================
// Chest Definitions (loaded from TOML)
// ============================================================================

/// An item that spawns in a chest slot by default (and can respawn)
#[derive(Debug, Clone, Deserialize)]
pub struct ChestSpawnItem {
    pub item_id: String,
    pub quantity: i32,
    pub slot: u8,
    pub respawn_secs: u64,
}

/// Definition of a chest type (loaded from data/chests.toml)
#[derive(Debug, Clone, Deserialize)]
pub struct ChestDef {
    #[serde(default = "default_slots")]
    pub slots: usize,
    #[serde(default)]
    pub spawn_items: Vec<ChestSpawnItem>,
}

fn default_slots() -> usize {
    10
}

// ============================================================================
// Chest Registry
// ============================================================================

/// Registry of all chest definitions loaded from TOML
pub struct ChestRegistry {
    defs: HashMap<String, ChestDef>,
}

impl ChestRegistry {
    pub fn new() -> Self {
        Self {
            defs: HashMap::new(),
        }
    }

    /// Load chest definitions from a TOML file
    pub fn load_from_file(&mut self, path: &Path) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read chests file {:?}: {}", path, e);
                return;
            }
        };

        let table: HashMap<String, ChestDef> = match toml::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                warn!("Failed to parse chests TOML {:?}: {}", path, e);
                return;
            }
        };

        for (id, mut def) in table {
            // Clamp slots to MAX_CHEST_SLOTS
            if def.slots > MAX_CHEST_SLOTS {
                warn!(
                    "Chest '{}' has {} slots, clamping to {}",
                    id, def.slots, MAX_CHEST_SLOTS
                );
                def.slots = MAX_CHEST_SLOTS;
            }

            // Validate spawn item slot indices
            def.spawn_items.retain(|item| {
                if (item.slot as usize) >= def.slots {
                    warn!(
                        "Chest '{}' spawn item '{}' has slot {} >= slots {}, skipping",
                        id, item.item_id, item.slot, def.slots
                    );
                    false
                } else {
                    true
                }
            });

            self.defs.insert(id, def);
        }

        info!("Loaded {} chest definitions", self.defs.len());
    }

    /// Get a chest definition by ID
    pub fn get(&self, id: &str) -> Option<&ChestDef> {
        self.defs.get(id)
    }
}

impl Default for ChestRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Overworld Chest Spawns (loaded from TOML)
// ============================================================================

/// A chest spawn point in the overworld
#[derive(Debug, Clone, Deserialize)]
pub struct ChestSpawn {
    pub chest_id: String,
    pub x: i32,
    pub y: i32,
}

/// File format for overworld chest spawns
#[derive(Debug, Clone, Deserialize)]
pub struct ChestSpawnsFile {
    #[serde(default)]
    pub chests: Vec<ChestSpawn>,
}

// ============================================================================
// Chest Runtime Instance
// ============================================================================

/// A live chest instance in the game world
pub struct ChestInstance {
    pub chest_def_id: String,
    pub slots: Vec<Option<InventorySlot>>,
    pub spawn_timers: HashMap<u8, Instant>,
    pub viewers: HashSet<String>,
}

impl ChestInstance {
    /// Create a new chest instance from a definition, pre-filling spawn items
    pub fn new(def_id: &str, def: &ChestDef) -> Self {
        let mut slots: Vec<Option<InventorySlot>> = vec![None; def.slots];

        // Pre-fill spawn items
        for spawn in &def.spawn_items {
            if (spawn.slot as usize) < slots.len() {
                slots[spawn.slot as usize] = Some(InventorySlot::new(
                    spawn.item_id.clone(),
                    spawn.quantity,
                ));
            }
        }

        Self {
            chest_def_id: def_id.to_string(),
            slots,
            spawn_timers: HashMap::new(),
            viewers: HashSet::new(),
        }
    }

    /// Calculate total value of items in the chest based on item registry base_price
    pub fn total_value(&self, item_registry: &ItemRegistry) -> i32 {
        self.slots
            .iter()
            .filter_map(|slot| slot.as_ref())
            .map(|slot| {
                let price = item_registry
                    .get(&slot.item_id)
                    .map(|def| def.base_price)
                    .unwrap_or(0);
                price * slot.quantity
            })
            .sum()
    }

    /// Serialize slots to JSON as Vec<(slot_index, item_id, quantity)> tuples
    pub fn slots_to_json(&self) -> String {
        let tuples: Vec<(u8, &str, i32)> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                slot.as_ref()
                    .map(|s| (i as u8, s.item_id.as_str(), s.quantity))
            })
            .collect();
        serde_json::to_string(&tuples).unwrap_or_else(|_| "[]".to_string())
    }

    /// Load slots from JSON (clears existing slots first)
    pub fn load_slots_from_json(&mut self, json: &str) {
        // Clear all slots
        for slot in &mut self.slots {
            *slot = None;
        }

        // Parse tuples: Vec<(slot_index, item_id, quantity)>
        if let Ok(tuples) = serde_json::from_str::<Vec<(u8, String, i32)>>(json) {
            for (slot_idx, item_id, quantity) in tuples {
                if (slot_idx as usize) < self.slots.len() && quantity > 0 {
                    self.slots[slot_idx as usize] =
                        Some(InventorySlot::new(item_id, quantity));
                }
            }
        }
    }
}

// ============================================================================
// Chest Manager
// ============================================================================

/// Manages all chest instances in the game world
pub struct ChestManager {
    pub chests: HashMap<String, ChestInstance>,
}

impl ChestManager {
    pub fn new() -> Self {
        Self {
            chests: HashMap::new(),
        }
    }

    /// Generate a key for an overworld chest at (x, y)
    pub fn overworld_key(x: i32, y: i32) -> String {
        format!("ow_{}_{}", x, y)
    }

    /// Generate a key for an interior chest at (interior_id, x, y)
    pub fn interior_key(interior_id: &str, x: i32, y: i32) -> String {
        format!("int_{}_{}_{}", interior_id, x, y)
    }

    /// Initialize all chest instances from registry data and spawn definitions
    pub fn init_from_registry(
        &mut self,
        registry: &ChestRegistry,
        overworld_spawns: &[ChestSpawn],
        interior_chests: &[(String, String, i32, i32)], // (interior_id, chest_id, x, y)
    ) {
        // Create overworld chest instances
        for spawn in overworld_spawns {
            if let Some(def) = registry.get(&spawn.chest_id) {
                let key = Self::overworld_key(spawn.x, spawn.y);
                self.chests
                    .insert(key, ChestInstance::new(&spawn.chest_id, def));
            } else {
                warn!(
                    "Overworld chest spawn references unknown chest_id '{}'",
                    spawn.chest_id
                );
            }
        }

        // Create interior chest instances
        for (interior_id, chest_id, x, y) in interior_chests {
            if let Some(def) = registry.get(chest_id) {
                let key = Self::interior_key(interior_id, *x, *y);
                self.chests
                    .insert(key, ChestInstance::new(chest_id, def));
            } else {
                warn!(
                    "Interior '{}' chest spawn references unknown chest_id '{}'",
                    interior_id, chest_id
                );
            }
        }

        info!("Initialized {} chest instances", self.chests.len());
    }

    /// Get an immutable reference to a chest by key
    pub fn get(&self, key: &str) -> Option<&ChestInstance> {
        self.chests.get(key)
    }

    /// Get a mutable reference to a chest by key
    pub fn get_mut(&mut self, key: &str) -> Option<&mut ChestInstance> {
        self.chests.get_mut(key)
    }

    /// Find an overworld chest at the given coordinates
    pub fn find_overworld(&self, x: i32, y: i32) -> Option<&ChestInstance> {
        let key = Self::overworld_key(x, y);
        self.chests.get(&key)
    }

    /// Find an interior chest at the given coordinates
    pub fn find_interior(&self, interior_id: &str, x: i32, y: i32) -> Option<&ChestInstance> {
        let key = Self::interior_key(interior_id, x, y);
        self.chests.get(&key)
    }

    /// Tick spawn timers: check if any spawn items should respawn
    pub fn tick_spawns(&mut self, registry: &ChestRegistry) {
        let now = Instant::now();

        for chest in self.chests.values_mut() {
            if chest.spawn_timers.is_empty() {
                continue;
            }

            let def = match registry.get(&chest.chest_def_id) {
                Some(d) => d,
                None => continue,
            };

            let mut to_respawn = Vec::new();
            for (&slot, &started_at) in &chest.spawn_timers {
                let elapsed = now.duration_since(started_at).as_secs();
                // Find the spawn item definition for this slot
                if let Some(spawn_item) = def.spawn_items.iter().find(|si| si.slot == slot) {
                    if elapsed >= spawn_item.respawn_secs {
                        to_respawn.push((slot, spawn_item.item_id.clone(), spawn_item.quantity));
                    }
                }
            }

            for (slot, item_id, quantity) in to_respawn {
                chest.spawn_timers.remove(&slot);
                if (slot as usize) < chest.slots.len() && chest.slots[slot as usize].is_none() {
                    chest.slots[slot as usize] =
                        Some(InventorySlot::new(item_id, quantity));
                }
            }
        }
    }

    /// Load saved chest data from the database
    pub fn load_saved_data(&mut self, saved: &HashMap<String, String>) {
        for (key, json) in saved {
            if let Some(chest) = self.chests.get_mut(key) {
                chest.load_slots_from_json(json);
            }
        }
    }

    /// Get all chest data for saving to the database
    pub fn get_save_data(&self) -> HashMap<String, String> {
        self.chests
            .iter()
            .map(|(key, chest)| (key.clone(), chest.slots_to_json()))
            .collect()
    }
}

impl Default for ChestManager {
    fn default() -> Self {
        Self::new()
    }
}
