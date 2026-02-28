use crate::farming::{CropConfig, FarmingSystem};
use rand::Rng;
use std::collections::HashMap;
use tracing::info;

/// Per-player state of an individual farming patch
#[derive(Debug, Clone)]
pub struct PlayerPatchState {
    pub crop_id: String,
    pub planted_at: u64, // timestamp ms
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
    pub state: String, // "empty", "planted", "growing", "harvestable"
    pub crop_id: String,
    pub growth_stage: u32,
    pub owner_id: String,
}

impl FarmingSystem {
    /// Restore a planted patch state (from database on startup)
    pub fn restore_patch(
        &mut self,
        patch_id: &str,
        player_id: &str,
        crop_id: &str,
        planted_at: u64,
    ) {
        if self.patches.contains_key(patch_id) {
            self.player_states.insert(
                (patch_id.to_string(), player_id.to_string()),
                PlayerPatchState {
                    crop_id: crop_id.to_string(),
                    planted_at,
                    last_broadcast_stage: 0,
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
    ) -> Result<(String, i64), String> {
        let (crop_id, crop) = self
            .crop_for_seed(seed_item_id)
            .ok_or_else(|| format!("{} is not a valid seed", seed_item_id))?;

        if farming_level < crop.level_required {
            return Err(format!(
                "You need Farming level {} to plant this",
                crop.level_required
            ));
        }

        let xp = crop.xp_planting;
        let crop_id = crop_id.to_string();

        if !self.patches.contains_key(patch_id) {
            return Err("Patch not found".to_string());
        }

        let plot_id = self.patches.get(patch_id).unwrap().plot;
        if !self.is_plot_unlocked(player_id, plot_id) {
            return Err("You haven't unlocked this allotment plot yet".to_string());
        }

        let key = (patch_id.to_string(), player_id.to_string());
        if self.player_states.contains_key(&key) {
            return Err("You already have something planted here".to_string());
        }

        self.player_states.insert(
            key,
            PlayerPatchState {
                crop_id: crop_id.clone(),
                planted_at: current_time,
                last_broadcast_stage: 0,
            },
        );

        Ok((crop_id, xp))
    }

    /// Try to harvest a crop from a patch (per-player)
    pub fn harvest_crop(
        &mut self,
        patch_id: &str,
        player_id: &str,
        current_time: u64,
        farming_level: i32,
    ) -> Result<HarvestResult, String> {
        if !self.patches.contains_key(patch_id) {
            return Err("Patch not found".to_string());
        }

        let key = (patch_id.to_string(), player_id.to_string());
        let state = self
            .player_states
            .get(&key)
            .ok_or_else(|| "Nothing to harvest".to_string())?;

        if !state.is_harvestable(&self.crops, current_time) {
            return Err("This crop is not ready to harvest yet".to_string());
        }

        let crop = self.crops.get(&state.crop_id).ok_or("Unknown crop type")?;

        let mut rng = rand::thread_rng();
        let base_amount = rng.gen_range(crop.harvest_amount_min..=crop.harvest_amount_max);
        let bonus = ((farming_level - crop.level_required) / 10).max(0);
        let amount = base_amount + bonus;
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
                let state_str = if is_harvestable {
                    "harvestable"
                } else {
                    "growing"
                };

                updates.push((
                    player_id.clone(),
                    PatchUpdate {
                        patch_id: patch_id.clone(),
                        state: state_str.to_string(),
                        crop_id: state.crop_id.clone(),
                        growth_stage: current_stage,
                        owner_id: player_id.clone(),
                    },
                ));
            }
        }

        updates
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
                let stage = state.growth_stage(&self.crops, current_time);
                let is_harvestable = state.is_harvestable(&self.crops, current_time);
                let state_str = if is_harvestable {
                    "harvestable"
                } else {
                    "growing"
                };
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
        }
    }

    fn patch(id: &str, plot: u32) -> FarmingPatch {
        FarmingPatch {
            id: id.to_string(),
            x: 0,
            y: 0,
            patch_type: "allotment".to_string(),
            plot,
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
            Err("You haven't unlocked this allotment plot yet".to_string())
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

        let harvest = system.harvest_crop("patch_1", "player", 0, 10).unwrap();

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
