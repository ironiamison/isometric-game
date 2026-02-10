//! Farming system - RuneScape-style allotment patch farming.
//!
//! Players plant seeds in fixed world patches, crops grow in real-time
//! through 4 stages, and can be harvested for produce and Farming XP.
//! Each player has their own instanced state per patch.

use std::collections::HashMap;
use std::path::Path;
use rand::Rng;
use serde::Deserialize;
use tracing::info;

// ---------------------------------------------------------------------------
// TOML deserialization structures
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
pub struct CropConfig {
    pub seed_item: String,
    pub produce_item: String,
    pub level_required: i32,
    pub growth_time_minutes: f32,
    pub growth_stages: u32,
    pub harvest_amount_min: i32,
    pub harvest_amount_max: i32,
    pub xp_planting: i64,
    pub xp_per_harvest: i64,
    pub seed_return_chance: f32,
}

#[derive(Deserialize, Debug, Clone)]
struct PatchLocationEntry {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub patch_type: String,
    #[serde(default = "default_plot")]
    pub plot: u32,
}

fn default_plot() -> u32 { 1 }

#[derive(Deserialize, Debug, Clone)]
struct PatchLocationsFile {
    patches: Vec<PatchLocationEntry>,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Per-player state of an individual farming patch
#[derive(Debug, Clone)]
pub struct PlayerPatchState {
    pub crop_id: String,
    pub planted_at: u64,  // timestamp ms
    /// Last growth stage sent to this player (for detecting transitions)
    pub last_broadcast_stage: u32,
}

impl PlayerPatchState {
    pub fn growth_stage(&self, crops: &HashMap<String, CropConfig>, current_time: u64) -> u32 {
        if let Some(crop) = crops.get(&self.crop_id) {
            let elapsed_ms = current_time.saturating_sub(self.planted_at);
            let total_ms = (crop.growth_time_minutes * 60.0 * 1000.0) as u64;
            if total_ms == 0 {
                return crop.growth_stages;
            }
            let stage = (elapsed_ms as f64 / total_ms as f64 * crop.growth_stages as f64) as u32;
            stage.min(crop.growth_stages)
        } else {
            0
        }
    }

    pub fn is_harvestable(&self, crops: &HashMap<String, CropConfig>, current_time: u64) -> bool {
        if let Some(crop) = crops.get(&self.crop_id) {
            self.growth_stage(crops, current_time) >= crop.growth_stages
        } else {
            false
        }
    }
}

/// A farming patch location in the world (shared by all players)
#[derive(Debug, Clone)]
pub struct FarmingPatch {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub patch_type: String,
    pub plot: u32,
}

/// Result of a harvest action
#[derive(Debug, Clone)]
pub struct HarvestResult {
    pub produce_item: String,
    pub amount: i32,
    pub xp_gained: i64,
    pub seed_returned: bool,
    pub seed_item: String,
}

/// Info about a patch state update to send to clients
#[derive(Debug, Clone)]
pub struct PatchUpdate {
    pub patch_id: String,
    pub state: String,       // "empty", "planted", "growing", "harvestable"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
}

/// The farming system — patch locations are global, patch states are per-player.
pub struct FarmingSystem {
    pub crops: HashMap<String, CropConfig>,
    /// Patch location definitions (shared)
    pub patches: HashMap<String, FarmingPatch>,
    /// Lookup: (x, y) -> patch_id
    pub patch_positions: HashMap<(i32, i32), String>,
    /// Per-player patch states: (patch_id, player_id) -> state
    pub player_states: HashMap<(String, String), PlayerPatchState>,
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {
            crops: HashMap::new(),
            patches: HashMap::new(),
            patch_positions: HashMap::new(),
            player_states: HashMap::new(),
        }
    }

    /// Load farming config from data directory
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let mut system = Self::new();

        // Load crop definitions
        let crops_path = data_dir.join("farming_patches.toml");
        if crops_path.exists() {
            let content = std::fs::read_to_string(&crops_path)
                .map_err(|e| format!("Failed to read farming_patches.toml: {}", e))?;
            let crops: HashMap<String, CropConfig> = toml::from_str(&content)
                .map_err(|e| format!("Failed to parse farming_patches.toml: {}", e))?;
            info!("Loaded {} crop definitions", crops.len());
            system.crops = crops;
        }

        // Load patch locations
        let locations_path = data_dir.join("farming_patch_locations.toml");
        if locations_path.exists() {
            let content = std::fs::read_to_string(&locations_path)
                .map_err(|e| format!("Failed to read farming_patch_locations.toml: {}", e))?;
            let locations: PatchLocationsFile = toml::from_str(&content)
                .map_err(|e| format!("Failed to parse farming_patch_locations.toml: {}", e))?;

            for entry in locations.patches {
                let patch = FarmingPatch {
                    id: entry.id.clone(),
                    x: entry.x,
                    y: entry.y,
                    patch_type: entry.patch_type,
                    plot: entry.plot,
                };
                system.patch_positions.insert((entry.x, entry.y), entry.id.clone());
                system.patches.insert(entry.id, patch);
            }
            info!("Loaded {} farming patch locations", system.patches.len());
        }

        Ok(system)
    }

    /// Restore a planted patch state (from database on startup)
    pub fn restore_patch(&mut self, patch_id: &str, player_id: &str, crop_id: &str, planted_at: u64) {
        if self.patches.contains_key(patch_id) {
            self.player_states.insert(
                (patch_id.to_string(), player_id.to_string()),
                PlayerPatchState {
                    crop_id: crop_id.to_string(),
                    planted_at,
                    last_broadcast_stage: 0,
                },
            );
            info!("Restored farming patch {} for player {} with crop {}", patch_id, player_id, crop_id);
        } else {
            info!("Skipping unknown patch {} during restore", patch_id);
        }
    }

    /// Get crop config for a seed item ID
    pub fn crop_for_seed(&self, seed_item_id: &str) -> Option<(&str, &CropConfig)> {
        self.crops.iter()
            .find(|(_, c)| c.seed_item == seed_item_id)
            .map(|(id, c)| (id.as_str(), c))
    }

    /// Try to plant a seed in a patch (per-player)
    pub fn plant_seed(
        &mut self,
        patch_id: &str,
        seed_item_id: &str,
        player_id: &str,
        farming_level: i32,
        current_time: u64,
    ) -> Result<(String, i64), String> {
        // Find crop for this seed
        let (crop_id, crop) = self.crop_for_seed(seed_item_id)
            .ok_or_else(|| format!("{} is not a valid seed", seed_item_id))?;

        // Check level requirement
        if farming_level < crop.level_required {
            return Err(format!("You need Farming level {} to plant this", crop.level_required));
        }

        let xp = crop.xp_planting;
        let crop_id = crop_id.to_string();

        // Check patch exists
        if !self.patches.contains_key(patch_id) {
            return Err("Patch not found".to_string());
        }

        // Check this player doesn't already have something planted here
        let key = (patch_id.to_string(), player_id.to_string());
        if self.player_states.contains_key(&key) {
            return Err("You already have something planted here".to_string());
        }

        // Plant the seed
        self.player_states.insert(key, PlayerPatchState {
            crop_id: crop_id.clone(),
            planted_at: current_time,
            last_broadcast_stage: 0,
        });

        Ok((crop_id, xp))
    }

    /// Try to harvest a crop from a patch (per-player)
    pub fn harvest_crop(
        &mut self,
        patch_id: &str,
        player_id: &str,
        current_time: u64,
    ) -> Result<HarvestResult, String> {
        if !self.patches.contains_key(patch_id) {
            return Err("Patch not found".to_string());
        }

        let key = (patch_id.to_string(), player_id.to_string());
        let state = self.player_states.get(&key)
            .ok_or_else(|| "Nothing to harvest".to_string())?;

        // Check if harvestable
        if !state.is_harvestable(&self.crops, current_time) {
            return Err("This crop is not ready to harvest yet".to_string());
        }

        let crop = self.crops.get(&state.crop_id)
            .ok_or("Unknown crop type")?;

        let mut rng = rand::thread_rng();
        let amount = rng.gen_range(crop.harvest_amount_min..=crop.harvest_amount_max);
        let xp = crop.xp_per_harvest * amount as i64;
        let seed_returned: f32 = rng.gen_range(0.0..1.0);
        let seed_returned = seed_returned < crop.seed_return_chance;

        let result = HarvestResult {
            produce_item: crop.produce_item.clone(),
            amount,
            xp_gained: xp,
            seed_returned,
            seed_item: crop.seed_item.clone(),
        };

        // Remove this player's state for this patch
        self.player_states.remove(&key);

        Ok(result)
    }

    /// Check all player patch states for growth stage changes, return updates to send per-player
    pub fn tick_growth(&mut self, current_time: u64) -> Vec<(String, PatchUpdate)> {
        let mut updates = Vec::new();

        for ((patch_id, player_id), state) in self.player_states.iter_mut() {
            let current_stage = state.growth_stage(&self.crops, current_time);

            if current_stage != state.last_broadcast_stage {
                state.last_broadcast_stage = current_stage;

                let is_harvestable = state.is_harvestable(&self.crops, current_time);
                let state_str = if is_harvestable { "harvestable" } else { "growing" };

                updates.push((player_id.clone(), PatchUpdate {
                    patch_id: patch_id.clone(),
                    state: state_str.to_string(),
                    crop_id: state.crop_id.clone(),
                    growth_stage: current_stage,
                    owner_id: player_id.clone(),
                }));
            }
        }

        updates
    }

    /// Get patch at a world position
    pub fn patch_at(&self, x: i32, y: i32) -> Option<&FarmingPatch> {
        self.patch_positions.get(&(x, y))
            .and_then(|id| self.patches.get(id))
    }

    /// Get all patch states for a specific player (for sending on connect)
    pub fn get_player_patch_states(&self, player_id: &str, current_time: u64) -> Vec<PatchUpdate> {
        let mut updates = Vec::new();

        for patch in self.patches.values() {
            let key = (patch.id.clone(), player_id.to_string());
            if let Some(state) = self.player_states.get(&key) {
                let stage = state.growth_stage(&self.crops, current_time);
                let is_harvestable = state.is_harvestable(&self.crops, current_time);
                let state_str = if is_harvestable { "harvestable" } else { "growing" };
                updates.push(PatchUpdate {
                    patch_id: patch.id.clone(),
                    state: state_str.to_string(),
                    crop_id: state.crop_id.clone(),
                    growth_stage: stage,
                    owner_id: player_id.to_string(),
                });
            } else {
                updates.push(PatchUpdate {
                    patch_id: patch.id.clone(),
                    state: "empty".to_string(),
                    crop_id: String::new(),
                    growth_stage: 0,
                    owner_id: String::new(),
                });
            }
        }

        updates
    }
}
