use std::collections::HashMap;
use std::path::Path;
use rand::Rng;

use super::definition::*;
use super::state::PlayerSlayerState;

pub struct SlayerRegistry {
    masters: HashMap<String, SlayerMasterDef>,
    master_by_prototype: HashMap<String, String>,
    rewards: Vec<SlayerRewardDef>,
    /// Maps monster_id -> required slayer level (only for gated monsters)
    slayer_requirements: HashMap<String, i32>,
}

impl SlayerRegistry {
    pub fn empty() -> Self {
        Self {
            masters: HashMap::new(),
            master_by_prototype: HashMap::new(),
            rewards: Vec::new(),
            slayer_requirements: HashMap::new(),
        }
    }

    pub fn load(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let slayer_dir = data_dir.join("slayer");

        let masters_path = slayer_dir.join("masters.toml");
        let masters_content = std::fs::read_to_string(&masters_path)?;
        let masters_file: SlayerMastersFile = toml::from_str(&masters_content)?;

        let rewards_path = slayer_dir.join("rewards.toml");
        let rewards_content = std::fs::read_to_string(&rewards_path)?;
        let rewards_file: SlayerRewardsFile = toml::from_str(&rewards_content)?;

        let mut masters = HashMap::new();
        let mut master_by_prototype = HashMap::new();
        let mut slayer_requirements = HashMap::new();

        for master in masters_file.masters {
            master_by_prototype.insert(master.entity_prototype.clone(), master.id.clone());
            // Collect slayer level requirements from tasks
            for task in &master.tasks {
                if task.slayer_level_required > 0 {
                    slayer_requirements.insert(task.monster_id.clone(), task.slayer_level_required);
                }
            }
            masters.insert(master.id.clone(), master);
        }

        tracing::info!("Loaded {} slayer masters, {} rewards, {} level-gated monsters",
            masters.len(), rewards_file.rewards.len(), slayer_requirements.len());

        Ok(Self {
            masters,
            master_by_prototype,
            rewards: rewards_file.rewards,
            slayer_requirements,
        })
    }

    pub fn get_master_by_prototype(&self, prototype_id: &str) -> Option<&SlayerMasterDef> {
        self.master_by_prototype.get(prototype_id)
            .and_then(|id| self.masters.get(id))
    }

    pub fn get_master(&self, master_id: &str) -> Option<&SlayerMasterDef> {
        self.masters.get(master_id)
    }

    pub fn get_rewards(&self) -> &[SlayerRewardDef] {
        &self.rewards
    }

    pub fn get_reward(&self, reward_id: &str) -> Option<&SlayerRewardDef> {
        self.rewards.iter().find(|r| r.id == reward_id)
    }

    pub fn get_slayer_requirement(&self, monster_id: &str) -> Option<i32> {
        self.slayer_requirements.get(monster_id).copied()
    }

    /// Assign a random task from the master's pool, respecting player state
    pub fn assign_task(
        &self,
        master_id: &str,
        player_slayer_level: i32,
        player_state: &PlayerSlayerState,
    ) -> Option<super::state::SlayerTask> {
        let master = self.masters.get(master_id)?;

        // Filter eligible tasks
        let eligible: Vec<&SlayerTaskDef> = master.tasks.iter().filter(|t| {
            // Must meet level requirement
            if t.slayer_level_required > player_slayer_level {
                return false;
            }
            // Must not be blocked
            if player_state.blocked_monsters.contains(&t.monster_id) {
                return false;
            }
            // Must be unlocked if requires_unlock
            if t.requires_unlock && !player_state.unlocked_monsters.contains(&t.monster_id) {
                return false;
            }
            true
        }).collect();

        if eligible.is_empty() {
            return None;
        }

        // Weighted random selection
        let total_weight: i32 = eligible.iter().map(|t| t.weight).sum();
        let mut rng = rand::thread_rng();
        let mut roll = rng.gen_range(0..total_weight);

        let mut chosen = eligible[0];
        for task in &eligible {
            roll -= task.weight;
            if roll < 0 {
                chosen = task;
                break;
            }
        }

        let kill_count = rng.gen_range(chosen.count_min..=chosen.count_max);

        Some(super::state::SlayerTask {
            monster_id: chosen.monster_id.clone(),
            display_name: chosen.display_name.clone(),
            kills_required: kill_count,
            kills_current: 0,
            xp_per_kill: chosen.xp_per_kill,
            master_id: master_id.to_string(),
            points_on_complete: master.points_per_task,
        })
    }
}
