//! Farming system - RuneScape-style allotment patch farming.
//!
//! Players plant seeds in fixed world patches, crops grow in real-time
//! through 4 stages, and can be harvested for produce and Farming XP.
//! Each player has their own instanced state per patch.

mod contracts;
mod patches;

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::info;

pub use self::contracts::{ContractDifficulty, FarmingContract};
pub use self::patches::{FarmingPatch, PLOT_REQUIREMENTS, PlayerPatchState, PlotRequirement};
#[allow(unused_imports)]
pub use self::patches::{HarvestResult, PatchUpdate};

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

fn default_plot() -> u32 {
    1
}

#[derive(Deserialize, Debug, Clone)]
struct PatchLocationsFile {
    patches: Vec<PatchLocationEntry>,
}

/// The farming system - patch locations are global, patch states are per-player.
pub struct FarmingSystem {
    pub crops: HashMap<String, CropConfig>,
    /// Patch location definitions (shared)
    pub patches: HashMap<String, FarmingPatch>,
    /// Lookup: (x, y) -> patch_id
    pub patch_positions: HashMap<(i32, i32), String>,
    /// Per-player patch states: (patch_id, player_id) -> state
    pub player_states: HashMap<(String, String), PlayerPatchState>,
    /// Per-player plot unlocks: player_id -> set of unlocked plot IDs
    pub player_plot_unlocks: HashMap<String, HashSet<u32>>,
    /// Active farming contracts: player_id -> contract
    pub contracts: HashMap<String, FarmingContract>,
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {
            crops: HashMap::new(),
            patches: HashMap::new(),
            patch_positions: HashMap::new(),
            player_states: HashMap::new(),
            player_plot_unlocks: HashMap::new(),
            contracts: HashMap::new(),
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
                system
                    .patch_positions
                    .insert((entry.x, entry.y), entry.id.clone());
                system.patches.insert(entry.id, patch);
            }
            info!("Loaded {} farming patch locations", system.patches.len());
        }

        Ok(system)
    }
}
