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
pub use self::patches::{FarmingPatch, PlayerPatchState};
#[allow(unused_imports)]
pub use self::patches::{HarvestResult, PatchHealth, PatchUpdate};

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
    /// Patch type this crop can be planted in: "allotment" | "herb" | "cactus" | "tree"
    #[serde(default = "default_category")]
    pub category: String,
    /// Minimum number of times the crop can be harvested before the patch empties
    #[serde(default = "default_lives")]
    pub lives_min: u32,
    /// Maximum number of harvest "lives"
    #[serde(default = "default_lives")]
    pub lives_max: u32,
    /// Per-growth-cycle probability the crop becomes diseased (0.0 = never)
    #[serde(default)]
    pub disease_chance: f32,
}

fn default_category() -> String {
    "allotment".to_string()
}

fn default_lives() -> u32 {
    1
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
    /// Empty patches treated with compost before planting: (patch_id, player_id)
    pub composted_empty_patches: HashSet<(String, String)>,
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
            composted_empty_patches: HashSet::new(),
        }
    }

    /// Load farming config from data directory. Patch *definitions* now come from the
    /// map (chunk `farmingPlots`, registered via `register_patch` at bootstrap); this
    /// only loads crop definitions.
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

        Ok(system)
    }

    /// Register a map-authored farming patch, indexing every tile of its footprint
    /// so a click anywhere on the bed resolves to this patch.
    pub fn register_patch(&mut self, patch: FarmingPatch) {
        for (tx, ty) in patches::patch_occupied_tiles(patch.x, patch.y, patch.width, patch.height) {
            self.patch_positions.insert((tx, ty), patch.id.clone());
        }
        self.patches.insert(patch.id.clone(), patch);
    }
}
