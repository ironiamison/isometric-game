//! Farming system - RuneScape-style allotment patch farming.
//!
//! Players plant seeds in fixed world patches, crops grow in real-time
//! through 4 stages, and can be harvested for produce and Farming XP.
//! Each player has their own instanced state per patch.

use std::collections::{HashMap, HashSet};
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

/// Plot unlock requirements
pub struct PlotRequirement {
    pub plot_id: u32,
    pub farming_level: i32,
    pub gold_cost: i32,
}

pub const PLOT_REQUIREMENTS: &[PlotRequirement] = &[
    PlotRequirement { plot_id: 2, farming_level: 15, gold_cost: 500 },
    PlotRequirement { plot_id: 3, farming_level: 30, gold_cost: 2000 },
    PlotRequirement { plot_id: 4, farming_level: 45, gold_cost: 5000 },
];

// ---------------------------------------------------------------------------
// Farming contracts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ContractDifficulty {
    Easy,
    Medium,
    Hard,
}

impl ContractDifficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContractDifficulty::Easy => "easy",
            ContractDifficulty::Medium => "medium",
            ContractDifficulty::Hard => "hard",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "easy" => Some(ContractDifficulty::Easy),
            "medium" => Some(ContractDifficulty::Medium),
            "hard" => Some(ContractDifficulty::Hard),
            _ => None,
        }
    }

    pub fn level_required(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 15,
            ContractDifficulty::Hard => 30,
        }
    }

    pub fn harvest_range(&self) -> (i32, i32) {
        match self {
            ContractDifficulty::Easy => (3, 5),
            ContractDifficulty::Medium => (6, 10),
            ContractDifficulty::Hard => (12, 18),
        }
    }

    pub fn xp_reward(&self) -> i64 {
        match self {
            ContractDifficulty::Easy => 150,
            ContractDifficulty::Medium => 500,
            ContractDifficulty::Hard => 1200,
        }
    }

    pub fn gold_reward(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 100,
            ContractDifficulty::Medium => 350,
            ContractDifficulty::Hard => 800,
        }
    }

    pub fn seed_reward_count(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 2,
            ContractDifficulty::Hard => 3,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ContractDifficulty::Easy => "Easy",
            ContractDifficulty::Medium => "Medium",
            ContractDifficulty::Hard => "Hard",
        }
    }
}

/// An active farming contract for a player
#[derive(Debug, Clone)]
pub struct FarmingContract {
    pub player_id: String,
    pub difficulty: ContractDifficulty,
    pub crop_id: String,
    pub amount_required: i32,
    pub amount_harvested: i32,
    pub created_at: u64,
}

impl FarmingContract {
    pub fn is_complete(&self) -> bool {
        self.amount_harvested >= self.amount_required
    }

    pub fn produce_item(&self, crops: &HashMap<String, CropConfig>) -> String {
        crops.get(&self.crop_id)
            .map(|c| c.produce_item.clone())
            .unwrap_or_else(|| self.crop_id.clone())
    }
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

        // Check plot is unlocked for this player
        let plot_id = self.patches.get(patch_id).unwrap().plot;
        if !self.is_plot_unlocked(player_id, plot_id) {
            return Err("You haven't unlocked this allotment plot yet".to_string());
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

    /// Check if a player has unlocked a specific plot (plot 1 is always unlocked)
    pub fn is_plot_unlocked(&self, player_id: &str, plot_id: u32) -> bool {
        if plot_id <= 1 { return true; }
        self.player_plot_unlocks
            .get(player_id)
            .map(|plots| plots.contains(&plot_id))
            .unwrap_or(false)
    }

    /// Unlock a plot for a player
    pub fn unlock_plot(&mut self, player_id: &str, plot_id: u32) {
        self.player_plot_unlocks
            .entry(player_id.to_string())
            .or_default()
            .insert(plot_id);
    }

    /// Get all unlocked plot IDs for a player (always includes plot 1)
    pub fn get_unlocked_plots(&self, player_id: &str) -> Vec<u32> {
        let mut plots = vec![1];
        if let Some(unlocked) = self.player_plot_unlocks.get(player_id) {
            plots.extend(unlocked.iter());
        }
        plots.sort();
        plots
    }

    /// Restore plot unlock from database
    pub fn restore_plot_unlock(&mut self, player_id: &str, plot_id: u32) {
        self.unlock_plot(player_id, plot_id);
    }

    /// Get all patch states for a specific player (for sending on connect)
    pub fn get_player_patch_states(&self, player_id: &str, current_time: u64) -> Vec<PatchUpdate> {
        let mut updates = Vec::new();

        for patch in self.patches.values() {
            // Only send patches for plots the player has unlocked
            if !self.is_plot_unlocked(player_id, patch.plot) {
                continue;
            }

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

    /// Generate a new farming contract for a player
    pub fn generate_contract(
        &mut self,
        player_id: &str,
        difficulty: &ContractDifficulty,
        farming_level: i32,
        current_time: u64,
    ) -> Result<&FarmingContract, String> {
        if self.contracts.contains_key(player_id) {
            return Err("You already have an active contract".to_string());
        }

        if farming_level < difficulty.level_required() {
            return Err(format!("You need Farming level {} for {} contracts",
                difficulty.level_required(), difficulty.display_name()));
        }

        let eligible_crops: Vec<(&str, &CropConfig)> = self.crops.iter()
            .filter(|(_, c)| c.level_required <= farming_level)
            .map(|(id, c)| (id.as_str(), c))
            .collect();

        if eligible_crops.is_empty() {
            return Err("No crops available for your level".to_string());
        }

        let mut rng = rand::thread_rng();
        let (crop_id, _) = eligible_crops[rng.gen_range(0..eligible_crops.len())];
        let (min, max) = difficulty.harvest_range();
        let amount = rng.gen_range(min..=max);

        let contract = FarmingContract {
            player_id: player_id.to_string(),
            difficulty: difficulty.clone(),
            crop_id: crop_id.to_string(),
            amount_required: amount,
            amount_harvested: 0,
            created_at: current_time,
        };

        self.contracts.insert(player_id.to_string(), contract);
        Ok(self.contracts.get(player_id).unwrap())
    }

    /// Record a harvest toward a player's active contract
    pub fn record_contract_harvest(&mut self, player_id: &str, crop_id: &str, amount: i32) -> Option<(i32, i32, bool)> {
        let contract = self.contracts.get_mut(player_id)?;
        if contract.crop_id != crop_id || contract.is_complete() {
            return None;
        }
        contract.amount_harvested = (contract.amount_harvested + amount).min(contract.amount_required);
        let complete = contract.is_complete();
        Some((contract.amount_harvested, contract.amount_required, complete))
    }

    /// Get a player's active contract
    pub fn get_contract(&self, player_id: &str) -> Option<&FarmingContract> {
        self.contracts.get(player_id)
    }

    /// Remove a player's contract (on completion or abandonment)
    pub fn remove_contract(&mut self, player_id: &str) -> Option<FarmingContract> {
        self.contracts.remove(player_id)
    }

    /// Restore a contract from database
    pub fn restore_contract(&mut self, player_id: &str, difficulty: &str, crop_id: &str, amount_required: i32, amount_harvested: i32, created_at: u64) {
        if let Some(diff) = ContractDifficulty::from_str(difficulty) {
            self.contracts.insert(player_id.to_string(), FarmingContract {
                player_id: player_id.to_string(),
                difficulty: diff,
                crop_id: crop_id.to_string(),
                amount_required,
                amount_harvested,
                created_at,
            });
        }
    }

    /// Pick a random seed item appropriate for the player's level
    pub fn random_seed_for_level(&self, farming_level: i32) -> Option<String> {
        let eligible: Vec<&CropConfig> = self.crops.values()
            .filter(|c| c.level_required <= farming_level)
            .collect();
        if eligible.is_empty() { return None; }
        let mut rng = rand::thread_rng();
        Some(eligible[rng.gen_range(0..eligible.len())].seed_item.clone())
    }
}
