use crate::farming::{CropConfig, FarmingSystem};
use rand::Rng;
use std::collections::HashMap;
use tracing::info;

/// Health of a planted patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatchHealth {
    #[default]
    Healthy,
    Diseased,
    Dead,
}

impl PatchHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            PatchHealth::Healthy => "healthy",
            PatchHealth::Diseased => "diseased",
            PatchHealth::Dead => "dead",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "diseased" => PatchHealth::Diseased,
            "dead" => PatchHealth::Dead,
            _ => PatchHealth::Healthy,
        }
    }
}

/// Milliseconds it takes a crop to advance one growth stage.
fn stage_duration_ms(crop: &CropConfig) -> u64 {
    let total_ms = (crop.growth_time_minutes * 60.0 * 1000.0) as u64;
    if crop.growth_stages == 0 {
        0
    } else {
        total_ms / crop.growth_stages as u64
    }
}

/// Per-player state of an individual farming patch
#[derive(Debug, Clone)]
pub struct PlayerPatchState {
    pub crop_id: String,
    pub planted_at: u64, // timestamp ms
    /// Last growth stage sent to this player (for detecting transitions)
    pub last_broadcast_stage: u32,
    /// Whether the crop is healthy, diseased, or dead
    pub health: PatchHealth,
    /// Whether this patch was treated with compost
    pub composted: bool,
    /// Remaining harvest "lives" before the patch empties
    pub lives_remaining: u32,
    /// Highest growth stage for which a disease roll has already been performed.
    /// When diseased/dead this also marks the frozen display stage.
    pub disease_cycle_marker: u32,
}

impl PlayerPatchState {
    /// Raw, time-based growth stage (ignores health).
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

    /// Stage to display to the client: frozen at the disease point when diseased/dead.
    pub fn display_stage(&self, crops: &HashMap<String, CropConfig>, current_time: u64) -> u32 {
        match self.health {
            PatchHealth::Healthy => self.growth_stage(crops, current_time),
            _ => {
                let stages = crops
                    .get(&self.crop_id)
                    .map(|c| c.growth_stages)
                    .unwrap_or(0);
                self.disease_cycle_marker.min(stages)
            }
        }
    }

    pub fn is_harvestable(&self, crops: &HashMap<String, CropConfig>, current_time: u64) -> bool {
        if self.health != PatchHealth::Healthy {
            return false;
        }
        if let Some(crop) = crops.get(&self.crop_id) {
            self.growth_stage(crops, current_time) >= crop.growth_stages
        } else {
            false
        }
    }

    /// The protocol state string for this patch.
    pub fn state_str(&self, crops: &HashMap<String, CropConfig>, current_time: u64) -> &'static str {
        match self.health {
            PatchHealth::Dead => "dead",
            PatchHealth::Diseased => "diseased",
            PatchHealth::Healthy => {
                if self.is_harvestable(crops, current_time) {
                    "harvestable"
                } else {
                    "growing"
                }
            }
        }
    }
}

/// A farming patch location in the world (shared by all players).
/// `(x, y)` is the NW anchor tile; the footprint spans `width`×`height` tiles.
#[derive(Debug, Clone)]
pub struct FarmingPatch {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub patch_type: String,
    pub plot: u32,
    pub width: u32,
    pub height: u32,
    /// Number of plants the patch holds (seeds consumed, yield multiplier).
    pub capacity: u32,
}

impl FarmingPatch {
    /// All tiles covered by this patch's footprint.
    pub fn occupied_tiles(&self) -> impl Iterator<Item = (i32, i32)> {
        patch_occupied_tiles(self.x, self.y, self.width, self.height)
    }

    /// Chebyshev distance from `(px, py)` to the nearest tile of the footprint.
    /// Mirrors `grid_distance_to_npc` for multi-tile NPCs.
    pub fn distance_to(&self, px: i32, py: i32) -> i32 {
        let closest_x = px.clamp(self.x, self.x + self.width as i32 - 1);
        let closest_y = py.clamp(self.y, self.y + self.height as i32 - 1);
        (px - closest_x).abs().max((py - closest_y).abs())
    }

    /// Whether `(px, py)` is orthogonally adjacent to the footprint perimeter.
    pub fn is_cardinally_adjacent(&self, px: i32, py: i32) -> bool {
        let closest_x = px.clamp(self.x, self.x + self.width as i32 - 1);
        let closest_y = py.clamp(self.y, self.y + self.height as i32 - 1);
        let dx = (px - closest_x).abs();
        let dy = (py - closest_y).abs();
        (dx + dy) == 1
    }
}

/// Footprint tiles for a patch anchored at `(x, y)`. Mirrors `npc_occupied_tiles`.
pub fn patch_occupied_tiles(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> impl Iterator<Item = (i32, i32)> {
    (0..height as i32).flat_map(move |dy| (0..width as i32).map(move |dx| (x + dx, y + dy)))
}

/// Result of a harvest action
#[derive(Debug, Clone)]
pub struct HarvestResult {
    pub produce_item: String,
    pub amount: i32,
    pub xp_gained: i64,
    pub seed_returned: bool,
    pub seed_item: String,
    /// Harvest lives left after this harvest
    pub lives_remaining: u32,
    /// Whether the patch emptied (no lives left)
    pub patch_emptied: bool,
}

/// Plot unlock requirements
pub struct PlotRequirement {
    pub plot_id: u32,
    pub farming_level: i32,
    pub gold_cost: i32,
}

pub const PLOT_REQUIREMENTS: &[PlotRequirement] = &[
    PlotRequirement {
        plot_id: 2,
        farming_level: 15,
        gold_cost: 500,
    },
    PlotRequirement {
        plot_id: 3,
        farming_level: 30,
        gold_cost: 2000,
    },
    PlotRequirement {
        plot_id: 4,
        farming_level: 45,
        gold_cost: 5000,
    },
];

/// Info about a patch state update to send to clients
#[derive(Debug, Clone)]
pub struct PatchUpdate {
    pub patch_id: String,
    pub state: String, // "empty", "growing", "harvestable", "diseased", "dead"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
    pub health: String,
    pub lives_remaining: u32,
    pub composted: bool,
    pub patch_type: String,
    /// Internal: frozen disease stage marker (not sent over the wire; used for persistence).
    pub disease_cycle_marker: u32,
}

impl FarmingSystem {
    /// Restore a planted patch state (from database on startup)
    #[allow(clippy::too_many_arguments)]
    pub fn restore_patch(
        &mut self,
        patch_id: &str,
        player_id: &str,
        crop_id: &str,
        planted_at: u64,
        lives_remaining: u32,
        health: &str,
        composted: bool,
        disease_cycle_marker: u32,
    ) {
        if self.patches.contains_key(patch_id) {
            self.player_states.insert(
                (patch_id.to_string(), player_id.to_string()),
                PlayerPatchState {
                    crop_id: crop_id.to_string(),
                    planted_at,
                    last_broadcast_stage: 0,
                    health: PatchHealth::from_str(health),
                    composted,
                    lives_remaining: lives_remaining.max(1),
                    disease_cycle_marker,
                },
            );
            info!(
                "Restored farming patch {} for player {} with crop {}",
                patch_id, player_id, crop_id
            );
        } else {
            info!("Skipping unknown patch {} during restore", patch_id);
        }
    }

    /// Get crop config for a seed item ID
    pub fn crop_for_seed(&self, seed_item_id: &str) -> Option<(&str, &CropConfig)> {
        self.crops
            .iter()
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
    ) -> Result<(String, i64, u32), String> {
        let (crop_id, crop) = self
            .crop_for_seed(seed_item_id)
            .ok_or_else(|| format!("{} is not a valid seed", seed_item_id))?;

        if farming_level < crop.level_required {
            return Err(format!(
                "You need Farming level {} to plant this",
                crop.level_required
            ));
        }

        // Copy out everything we need before the immutable borrow of `self` ends.
        let xp_planting = crop.xp_planting;
        let category = crop.category.clone();
        let lives_min = crop.lives_min;
        let lives_max = crop.lives_max;
        let crop_id = crop_id.to_string();

        let patch = self
            .patches
            .get(patch_id)
            .ok_or_else(|| "Patch not found".to_string())?;
        let patch_type = patch.patch_type.clone();
        let plot_id = patch.plot;
        let capacity = patch.capacity.max(1);
        // A bed of N plants gives N× the planting XP (and consumes N seeds).
        let xp = xp_planting * capacity as i64;

        if category != patch_type {
            return Err(format!("You can't plant that in a {} patch", patch_type));
        }

        if !self.is_plot_unlocked(player_id, plot_id) {
            return Err("You haven't unlocked this plot yet".to_string());
        }

        let key = (patch_id.to_string(), player_id.to_string());
        if self.player_states.contains_key(&key) {
            return Err("You already have something planted here".to_string());
        }

        let composted = self.composted_empty_patches.remove(&key);
        let base_lives = if lives_max <= lives_min {
            lives_min
        } else {
            rand::thread_rng().gen_range(lives_min..=lives_max)
        };
        let lives_remaining = (base_lives + if composted { 1 } else { 0 }).max(1);

        self.player_states.insert(
            key,
            PlayerPatchState {
                crop_id: crop_id.clone(),
                planted_at: current_time,
                last_broadcast_stage: 0,
                health: PatchHealth::Healthy,
                composted,
                lives_remaining,
                disease_cycle_marker: 0,
            },
        );

        Ok((crop_id, xp, capacity))
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
        let state = self
            .player_states
            .get(&key)
            .ok_or_else(|| "Nothing to harvest".to_string())?;

        match state.health {
            PatchHealth::Diseased => return Err("This crop is diseased.".to_string()),
            PatchHealth::Dead => return Err("This crop has died.".to_string()),
            PatchHealth::Healthy => {}
        }

        if !state.is_harvestable(&self.crops, current_time) {
            return Err("This crop is not ready to harvest yet".to_string());
        }

        let crop = self.crops.get(&state.crop_id).ok_or("Unknown crop type")?;
        // A multi-plant bed yields capacity× per harvest.
        let capacity = self
            .patches
            .get(patch_id)
            .map(|p| p.capacity.max(1))
            .unwrap_or(1) as i32;

        let mut rng = rand::thread_rng();
        // Yield is per-harvest; total volume comes from the patch's harvest lives, so we no
        // longer add a per-harvest level bonus (it stacked across every life and ballooned).
        let amount = rng.gen_range(crop.harvest_amount_min..=crop.harvest_amount_max) * capacity;
        let xp = crop.xp_per_harvest * amount as i64;
        let seed_returned: f32 = rng.gen_range(0.0..1.0);
        let seed_returned = seed_returned < crop.seed_return_chance;
        let produce_item = crop.produce_item.clone();
        let seed_item = crop.seed_item.clone();

        // Spend a harvest life; the patch only empties once lives run out.
        let state = self.player_states.get_mut(&key).unwrap();
        state.lives_remaining = state.lives_remaining.saturating_sub(1);
        let lives_remaining = state.lives_remaining;
        let patch_emptied = lives_remaining == 0;
        if patch_emptied {
            self.player_states.remove(&key);
        }

        Ok(HarvestResult {
            produce_item,
            amount,
            xp_gained: xp,
            seed_returned,
            seed_item,
            lives_remaining,
            patch_emptied,
        })
    }

    /// Check all player patch states for growth/health changes, return updates to send per-player.
    pub fn tick_growth(&mut self, current_time: u64) -> Vec<(String, PatchUpdate)> {
        let mut updates = Vec::new();
        let mut rng = rand::thread_rng();

        for ((patch_id, player_id), state) in self.player_states.iter_mut() {
            let Some(crop) = self.crops.get(&state.crop_id) else {
                continue;
            };
            let stages = crop.growth_stages;
            let mut changed = false;

            match state.health {
                PatchHealth::Dead => {
                    // Terminal; the "dead" transition was already broadcast.
                }
                PatchHealth::Diseased => {
                    // A diseased crop dies once another full growth cycle's worth of
                    // time elapses without a cure (raw stage advances past the frozen stage).
                    if state.growth_stage(&self.crops, current_time) > state.disease_cycle_marker {
                        state.health = PatchHealth::Dead;
                        changed = true;
                    }
                }
                PatchHealth::Healthy => {
                    let stage = state.growth_stage(&self.crops, current_time);
                    // Roll for disease at most once per growth cycle, while still growing.
                    if stage > state.disease_cycle_marker && stage < stages {
                        state.disease_cycle_marker = stage;
                        let chance = crop.disease_chance * if state.composted { 0.5 } else { 1.0 };
                        if chance > 0.0 && rng.gen_range(0.0..1.0) < chance {
                            state.health = PatchHealth::Diseased;
                            changed = true;
                        }
                    }
                    if !changed && stage != state.last_broadcast_stage {
                        state.last_broadcast_stage = stage;
                        changed = true;
                    }
                }
            }

            if changed {
                let patch_type = self
                    .patches
                    .get(patch_id)
                    .map(|p| p.patch_type.clone())
                    .unwrap_or_default();
                updates.push((
                    player_id.clone(),
                    PatchUpdate {
                        patch_id: patch_id.clone(),
                        state: state.state_str(&self.crops, current_time).to_string(),
                        crop_id: state.crop_id.clone(),
                        growth_stage: state.display_stage(&self.crops, current_time),
                        owner_id: player_id.clone(),
                        health: state.health.as_str().to_string(),
                        lives_remaining: state.lives_remaining,
                        composted: state.composted,
                        patch_type,
                        disease_cycle_marker: state.disease_cycle_marker,
                    },
                ));
            }
        }

        updates
    }

    /// Cure a diseased patch, resuming growth from the frozen stage.
    pub fn cure_patch(
        &mut self,
        patch_id: &str,
        player_id: &str,
        current_time: u64,
    ) -> Result<(), String> {
        let key = (patch_id.to_string(), player_id.to_string());
        let state = self
            .player_states
            .get_mut(&key)
            .ok_or_else(|| "Nothing planted here".to_string())?;
        if state.health != PatchHealth::Diseased {
            return Err("This crop isn't diseased.".to_string());
        }
        let crop = self
            .crops
            .get(&state.crop_id)
            .ok_or_else(|| "Unknown crop type".to_string())?;
        let frozen = state.disease_cycle_marker;
        let stage_ms = stage_duration_ms(crop);
        // Rebase planted_at so the raw stage equals the frozen stage and resumes forward.
        state.planted_at = current_time.saturating_sub(frozen as u64 * stage_ms);
        state.last_broadcast_stage = frozen;
        state.health = PatchHealth::Healthy;
        Ok(())
    }

    /// Clear a dead patch, returning it to empty.
    pub fn clear_patch(&mut self, patch_id: &str, player_id: &str) -> Result<(), String> {
        let key = (patch_id.to_string(), player_id.to_string());
        let state = self
            .player_states
            .get(&key)
            .ok_or_else(|| "Nothing to clear here".to_string())?;
        if state.health != PatchHealth::Dead {
            return Err("There's nothing to clear here.".to_string());
        }
        self.player_states.remove(&key);
        Ok(())
    }

    /// Apply compost to a patch (empty or healthy & growing). Returns true if applied.
    pub fn apply_compost(&mut self, patch_id: &str, player_id: &str) -> Result<(), String> {
        if !self.patches.contains_key(patch_id) {
            return Err("Patch not found".to_string());
        }
        let key = (patch_id.to_string(), player_id.to_string());
        if let Some(state) = self.player_states.get_mut(&key) {
            if state.composted {
                return Err("This patch is already treated with compost.".to_string());
            }
            if state.health != PatchHealth::Healthy {
                return Err("You can't compost this patch right now.".to_string());
            }
            state.composted = true;
        } else {
            if self.composted_empty_patches.contains(&key) {
                return Err("This patch is already treated with compost.".to_string());
            }
            self.composted_empty_patches.insert(key);
        }
        Ok(())
    }

    /// Whether a (currently empty) patch has been pre-treated with compost.
    pub fn is_empty_patch_composted(&self, patch_id: &str, player_id: &str) -> bool {
        self.composted_empty_patches
            .contains(&(patch_id.to_string(), player_id.to_string()))
    }

    /// Clone of the per-player state for a patch, if anything is planted (for persistence).
    pub fn get_state(&self, patch_id: &str, player_id: &str) -> Option<PlayerPatchState> {
        self.player_states
            .get(&(patch_id.to_string(), player_id.to_string()))
            .cloned()
    }

    /// Build the current client-facing update for a single patch (empty if nothing planted).
    pub fn patch_update_for(
        &self,
        patch_id: &str,
        player_id: &str,
        current_time: u64,
    ) -> PatchUpdate {
        let patch_type = self
            .patches
            .get(patch_id)
            .map(|p| p.patch_type.clone())
            .unwrap_or_default();
        let key = (patch_id.to_string(), player_id.to_string());
        if let Some(state) = self.player_states.get(&key) {
            PatchUpdate {
                patch_id: patch_id.to_string(),
                state: state.state_str(&self.crops, current_time).to_string(),
                crop_id: state.crop_id.clone(),
                growth_stage: state.display_stage(&self.crops, current_time),
                owner_id: player_id.to_string(),
                health: state.health.as_str().to_string(),
                lives_remaining: state.lives_remaining,
                composted: state.composted,
                patch_type,
                disease_cycle_marker: state.disease_cycle_marker,
            }
        } else {
            PatchUpdate {
                patch_id: patch_id.to_string(),
                state: "empty".to_string(),
                crop_id: String::new(),
                growth_stage: 0,
                owner_id: String::new(),
                health: PatchHealth::Healthy.as_str().to_string(),
                lives_remaining: 0,
                composted: self.composted_empty_patches.contains(&key),
                patch_type,
                disease_cycle_marker: 0,
            }
        }
    }

    /// Get patch at a world position
    pub fn patch_at(&self, x: i32, y: i32) -> Option<&FarmingPatch> {
        self.patch_positions
            .get(&(x, y))
            .and_then(|id| self.patches.get(id))
    }

    /// Check if a player has unlocked a specific plot (plot 1 is always unlocked)
    pub fn is_plot_unlocked(&self, player_id: &str, plot_id: u32) -> bool {
        if plot_id <= 1 {
            return true;
        }
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
            if !self.is_plot_unlocked(player_id, patch.plot) {
                continue;
            }

            let key = (patch.id.clone(), player_id.to_string());
            if let Some(state) = self.player_states.get(&key) {
                updates.push(PatchUpdate {
                    patch_id: patch.id.clone(),
                    state: state.state_str(&self.crops, current_time).to_string(),
                    crop_id: state.crop_id.clone(),
                    growth_stage: state.display_stage(&self.crops, current_time),
                    owner_id: player_id.to_string(),
                    health: state.health.as_str().to_string(),
                    lives_remaining: state.lives_remaining,
                    composted: state.composted,
                    patch_type: patch.patch_type.clone(),
                    disease_cycle_marker: state.disease_cycle_marker,
                });
            } else {
                updates.push(PatchUpdate {
                    patch_id: patch.id.clone(),
                    state: "empty".to_string(),
                    crop_id: String::new(),
                    growth_stage: 0,
                    owner_id: String::new(),
                    health: PatchHealth::Healthy.as_str().to_string(),
                    lives_remaining: 0,
                    composted: self.composted_empty_patches.contains(&key),
                    patch_type: patch.patch_type.clone(),
                    disease_cycle_marker: 0,
                });
            }
        }

        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crop(seed_item: &str, produce_item: &str, growth_time_minutes: f32) -> CropConfig {
        CropConfig {
            seed_item: seed_item.to_string(),
            produce_item: produce_item.to_string(),
            level_required: 1,
            growth_time_minutes,
            growth_stages: 4,
            harvest_amount_min: 2,
            harvest_amount_max: 2,
            xp_planting: 10,
            xp_per_harvest: 5,
            seed_return_chance: 1.0,
            category: "allotment".to_string(),
            lives_min: 1,
            lives_max: 1,
            disease_chance: 0.0,
        }
    }

    fn patch(id: &str, plot: u32) -> FarmingPatch {
        FarmingPatch {
            id: id.to_string(),
            x: 0,
            y: 0,
            patch_type: "allotment".to_string(),
            plot,
            width: 1,
            height: 1,
            capacity: 1,
        }
    }

    #[test]
    fn plant_seed_rejects_locked_plots() {
        let mut system = FarmingSystem::new();
        system.crops.insert(
            "carrot".to_string(),
            crop("carrot_seed", "carrot_item", 1.0),
        );
        system
            .patches
            .insert("patch_2".to_string(), patch("patch_2", 2));

        let result = system.plant_seed("patch_2", "carrot_seed", "player", 10, 0);

        assert_eq!(
            result,
            Err("You haven't unlocked this plot yet".to_string())
        );
    }

    #[test]
    fn harvest_crop_returns_expected_rewards_and_clears_patch() {
        let mut system = FarmingSystem::new();
        system.crops.insert(
            "carrot".to_string(),
            crop("carrot_seed", "carrot_item", 0.0),
        );
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));

        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        let harvest = system.harvest_crop("patch_1", "player", 0).unwrap();

        assert_eq!(harvest.produce_item, "carrot_item");
        assert_eq!(harvest.amount, 2);
        assert_eq!(harvest.xp_gained, 10);
        assert!(harvest.seed_returned);
        assert_eq!(harvest.seed_item, "carrot_seed");
        assert!(system.player_states.is_empty());
    }

    #[test]
    fn tick_growth_only_emits_stage_changes_once() {
        let mut system = FarmingSystem::new();
        system.crops.insert(
            "carrot".to_string(),
            crop("carrot_seed", "carrot_item", 1.0),
        );
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));
        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        assert!(system.tick_growth(0).is_empty());

        let first_updates = system.tick_growth(15_000);
        assert_eq!(first_updates.len(), 1);
        assert_eq!(first_updates[0].0, "player");
        assert_eq!(first_updates[0].1.patch_id, "patch_1");
        assert_eq!(first_updates[0].1.state, "growing");
        assert_eq!(first_updates[0].1.growth_stage, 1);

        assert!(system.tick_growth(15_000).is_empty());
    }

    fn herb_patch(id: &str) -> FarmingPatch {
        FarmingPatch {
            id: id.to_string(),
            x: 0,
            y: 0,
            patch_type: "herb".to_string(),
            plot: 1,
            width: 1,
            height: 1,
            capacity: 1,
        }
    }

    #[test]
    fn plant_seed_rejects_wrong_category() {
        let mut system = FarmingSystem::new();
        // carrot is an allotment crop by default
        system
            .crops
            .insert("carrot".to_string(), crop("carrot_seed", "carrot_item", 1.0));
        system
            .patches
            .insert("herb_1".to_string(), herb_patch("herb_1"));

        let result = system.plant_seed("herb_1", "carrot_seed", "player", 10, 0);
        assert_eq!(
            result,
            Err("You can't plant that in a herb patch".to_string())
        );
    }

    #[test]
    fn harvest_lives_keep_patch_until_exhausted() {
        let mut system = FarmingSystem::new();
        let mut c = crop("carrot_seed", "carrot_item", 0.0);
        c.lives_min = 3;
        c.lives_max = 3;
        system.crops.insert("carrot".to_string(), c);
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));
        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        let h1 = system.harvest_crop("patch_1", "player", 0).unwrap();
        assert_eq!(h1.lives_remaining, 2);
        assert!(!h1.patch_emptied);
        assert!(!system.player_states.is_empty());

        let h2 = system.harvest_crop("patch_1", "player", 0).unwrap();
        assert_eq!(h2.lives_remaining, 1);
        assert!(!h2.patch_emptied);

        let h3 = system.harvest_crop("patch_1", "player", 0).unwrap();
        assert_eq!(h3.lives_remaining, 0);
        assert!(h3.patch_emptied);
        assert!(system.player_states.is_empty());
    }

    #[test]
    fn disease_progresses_to_dead_and_blocks_harvest() {
        let mut system = FarmingSystem::new();
        let mut c = crop("carrot_seed", "carrot_item", 1.0); // 60s, 4 stages, 15s/stage
        c.disease_chance = 1.0; // always diseases at first cycle boundary
        system.crops.insert("carrot".to_string(), c);
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));
        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        // First cycle boundary -> diseased
        let updates = system.tick_growth(15_000);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].1.state, "diseased");
        assert!(system.harvest_crop("patch_1", "player", 60_000).is_err());

        // Next cycle without a cure -> dead
        let updates = system.tick_growth(30_000);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].1.state, "dead");
    }

    #[test]
    fn cure_restores_health_and_allows_growth() {
        let mut system = FarmingSystem::new();
        let mut c = crop("carrot_seed", "carrot_item", 1.0);
        c.disease_chance = 1.0;
        system.crops.insert("carrot".to_string(), c);
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));
        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        system.tick_growth(15_000); // diseased at stage 1
        system.cure_patch("patch_1", "player", 15_000).unwrap();

        let key = ("patch_1".to_string(), "player".to_string());
        assert_eq!(
            system.player_states.get(&key).unwrap().health,
            PatchHealth::Healthy
        );
        // After curing it should eventually grow to harvestable.
        assert!(
            system
                .player_states
                .get(&key)
                .unwrap()
                .is_harvestable(&system.crops, 120_000)
        );
    }

    #[test]
    fn compost_adds_a_harvest_life() {
        let mut system = FarmingSystem::new();
        let mut c = crop("carrot_seed", "carrot_item", 0.0);
        c.lives_min = 2;
        c.lives_max = 2;
        system.crops.insert("carrot".to_string(), c);
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));

        system.apply_compost("patch_1", "player").unwrap();
        system
            .plant_seed("patch_1", "carrot_seed", "player", 10, 0)
            .unwrap();

        let key = ("patch_1".to_string(), "player".to_string());
        let state = system.player_states.get(&key).unwrap();
        assert_eq!(state.lives_remaining, 3); // 2 base + 1 compost
        assert!(state.composted);
        // pre-plant compost flag is consumed at plant time
        assert!(!system.is_empty_patch_composted("patch_1", "player"));
    }

    fn sized_patch(id: &str, w: u32, h: u32, cap: u32) -> FarmingPatch {
        FarmingPatch {
            id: id.to_string(),
            x: 0,
            y: 0,
            patch_type: "allotment".to_string(),
            plot: 1,
            width: w,
            height: h,
            capacity: cap,
        }
    }

    #[test]
    fn patch_occupied_tiles_covers_footprint() {
        let tiles: Vec<(i32, i32)> = patch_occupied_tiles(2, 3, 2, 2).collect();
        assert_eq!(tiles, vec![(2, 3), (3, 3), (2, 4), (3, 4)]);
    }

    #[test]
    fn distance_and_cardinal_adjacency_use_footprint() {
        let p = sized_patch("bed", 2, 2, 4); // footprint (0,0)..(1,1)
        // Orthogonally next to the west edge.
        assert_eq!(p.distance_to(-1, 0), 1);
        assert!(p.is_cardinally_adjacent(-1, 0));
        // Standing on the footprint is distance 0, not adjacent.
        assert_eq!(p.distance_to(1, 1), 0);
        assert!(!p.is_cardinally_adjacent(1, 1));
        // Diagonally off a corner: distance 1 but not cardinal.
        assert_eq!(p.distance_to(2, 2), 1);
        assert!(!p.is_cardinally_adjacent(2, 2));
    }

    #[test]
    fn capacity_scales_seeds_xp_and_yield() {
        let mut system = FarmingSystem::new();
        let mut c = crop("carrot_seed", "carrot_item", 0.0);
        c.harvest_amount_min = 2;
        c.harvest_amount_max = 2;
        c.xp_planting = 5;
        system.crops.insert("carrot".to_string(), c);
        system.patches.insert("bed".to_string(), sized_patch("bed", 2, 2, 4));

        // Planting a capacity-4 bed gives 4× planting XP and reports capacity.
        let (_, plant_xp, capacity) = system
            .plant_seed("bed", "carrot_seed", "player", 10, 0)
            .unwrap();
        assert_eq!(capacity, 4);
        assert_eq!(plant_xp, 20); // 5 × 4

        // Each harvest yields amount × capacity.
        let h = system.harvest_crop("bed", "player", 0).unwrap();
        assert_eq!(h.amount, 8); // 2 × 4
    }

    #[test]
    fn get_player_patch_states_only_includes_unlocked_plots() {
        let mut system = FarmingSystem::new();
        system
            .patches
            .insert("patch_1".to_string(), patch("patch_1", 1));
        system
            .patches
            .insert("patch_2".to_string(), patch("patch_2", 2));

        let locked_updates = system.get_player_patch_states("player", 0);
        assert_eq!(locked_updates.len(), 1);
        assert_eq!(locked_updates[0].patch_id, "patch_1");
        assert_eq!(locked_updates[0].state, "empty");

        system.unlock_plot("player", 2);
        let unlocked_updates = system.get_player_patch_states("player", 0);
        assert_eq!(unlocked_updates.len(), 2);
    }
}
