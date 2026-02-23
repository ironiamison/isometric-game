//! Woodcutting skill system.
//!
//! Allows players to chop trees on the map. Trees are identified by their GID
//! from the tileset. When depleted, trees disappear and respawn after a delay.

use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

// ---------------------------------------------------------------------------
// TOML deserialization structures
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
pub struct TreeTypeConfig {
    pub gids: Vec<u32>,
    pub level_required: i32,
    pub log_item_id: String,
    pub xp_per_log: i64,
    /// Chance to successfully get a log on each swing (0.0 - 1.0)
    #[serde(default = "default_success_chance")]
    pub success_chance: f32,
    pub depletion_chance: f32,
    pub respawn_time_min: u64,
    pub respawn_time_max: u64,
}

fn default_success_chance() -> f32 {
    0.25 // 25% chance per swing by default
}

#[derive(Deserialize, Debug)]
struct TreeTypesFile {
    trees: HashMap<String, TreeTypeConfig>,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DepletedTree {
    pub gid: u32,
    pub tree_type_id: String,
    pub depleted_at: u64,
    pub respawn_at: u64,
}

#[derive(Debug, Clone)]
pub struct PlayerWoodcuttingState {
    pub tree_x: i32,
    pub tree_y: i32,
    pub tree_type_id: String,
    pub last_chop_tick: u64,
}

/// Swing interval in milliseconds (how often the player swings their axe)
/// Base swing interval - matches client attack cooldown (800ms)
pub const SWING_INTERVAL_MS: u64 = 800;

#[derive(Debug, Clone)]
pub struct ChopResult {
    /// Whether this swing successfully got a log
    pub success: bool,
    /// Item ID of the log (only meaningful if success=true)
    pub log_item_id: String,
    /// XP gained (only meaningful if success=true)
    pub xp_gained: i64,
    /// Whether the tree was depleted (only on successful chops)
    pub tree_depleted: bool,
    pub respawn_delay_ms: Option<u64>,
    /// Tree type ID (e.g., "oak", "willow") - for quest tracking
    pub tree_type_id: String,
}

#[derive(Debug, Clone)]
pub struct TreeRespawnEvent {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
}

// ---------------------------------------------------------------------------
// WoodcuttingSystem
// ---------------------------------------------------------------------------

pub struct WoodcuttingSystem {
    /// Tree type ID -> config
    pub tree_types: HashMap<String, TreeTypeConfig>,
    /// GID -> tree type ID mapping for fast lookup
    pub gid_to_tree_type: HashMap<u32, String>,
    /// (x, y) -> depleted tree state
    pub depleted_trees: HashMap<(i32, i32), DepletedTree>,
    /// Player ID -> woodcutting state
    pub player_states: HashMap<String, PlayerWoodcuttingState>,
}

impl WoodcuttingSystem {
    pub fn new() -> Self {
        Self {
            tree_types: HashMap::new(),
            gid_to_tree_type: HashMap::new(),
            depleted_trees: HashMap::new(),
            player_states: HashMap::new(),
        }
    }

    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("tree_types.toml");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read tree_types.toml: {}", e))?;

        let file: TreeTypesFile = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse tree_types.toml: {}", e))?;

        let mut gid_to_tree_type = HashMap::new();
        for (type_id, config) in &file.trees {
            for gid in &config.gids {
                gid_to_tree_type.insert(*gid, type_id.clone());
            }
        }

        info!(
            "Loaded {} tree types with {} GID mappings",
            file.trees.len(),
            gid_to_tree_type.len()
        );

        Ok(Self {
            tree_types: file.trees,
            gid_to_tree_type,
            depleted_trees: HashMap::new(),
            player_states: HashMap::new(),
        })
    }

    /// Check if a GID is a tree that can be chopped
    pub fn is_tree_gid(&self, gid: u32) -> bool {
        self.gid_to_tree_type.contains_key(&gid)
    }

    /// Get the tree type config for a GID
    pub fn get_tree_type(&self, gid: u32) -> Option<&TreeTypeConfig> {
        let type_id = self.gid_to_tree_type.get(&gid)?;
        self.tree_types.get(type_id)
    }

    /// Get tree type ID for a GID
    pub fn get_tree_type_id(&self, gid: u32) -> Option<&String> {
        self.gid_to_tree_type.get(&gid)
    }

    /// Check if a tree at given position is depleted
    pub fn is_tree_depleted(&self, x: i32, y: i32) -> bool {
        self.depleted_trees.contains_key(&(x, y))
    }

    /// Start woodcutting at a tree position
    pub fn start_woodcutting(
        &mut self,
        player_id: &str,
        tree_x: i32,
        tree_y: i32,
        tree_gid: u32,
        player_woodcutting_level: i32,
        current_time: u64,
    ) -> Result<String, String> {
        // Check if tree is depleted
        if self.is_tree_depleted(tree_x, tree_y) {
            return Err("This tree has been chopped down".to_string());
        }

        // Get tree type
        let tree_type_id = self
            .gid_to_tree_type
            .get(&tree_gid)
            .ok_or_else(|| "Not a tree".to_string())?
            .clone();

        let tree_config = self
            .tree_types
            .get(&tree_type_id)
            .ok_or_else(|| format!("Unknown tree type: {}", tree_type_id))?;

        // Check level requirement
        if player_woodcutting_level < tree_config.level_required {
            return Err(format!(
                "Requires Woodcutting level {} (you are level {})",
                tree_config.level_required, player_woodcutting_level
            ));
        }

        // Stop any existing woodcutting
        self.player_states.remove(player_id);

        // Start woodcutting
        self.player_states.insert(
            player_id.to_string(),
            PlayerWoodcuttingState {
                tree_x,
                tree_y,
                tree_type_id: tree_type_id.clone(),
                last_chop_tick: current_time,
            },
        );

        info!(
            "Player {} started woodcutting {} at ({}, {})",
            player_id, tree_type_id, tree_x, tree_y
        );

        Ok(tree_type_id)
    }

    /// Stop woodcutting for a player
    pub fn stop_woodcutting(&mut self, player_id: &str) -> Option<PlayerWoodcuttingState> {
        let state = self.player_states.remove(player_id)?;
        info!("Player {} stopped woodcutting", player_id);
        Some(state)
    }

    /// Check if a player is currently woodcutting
    pub fn is_woodcutting(&self, player_id: &str) -> bool {
        self.player_states.contains_key(player_id)
    }

    /// Get the set of player IDs currently woodcutting
    pub fn woodcutting_player_ids(&self) -> std::collections::HashSet<String> {
        self.player_states.keys().cloned().collect()
    }

    /// Tick woodcutting for a player, returning a ChopResult on each swing
    /// The result indicates whether the swing was successful (got a log) or a miss
    pub fn tick_woodcutting(
        &mut self,
        player_id: &str,
        current_time: u64,
        chop_speed_multiplier: f32,
        chop_success_bonus: f32,
        player_woodcutting_level: i32,
    ) -> Option<ChopResult> {
        let state = self.player_states.get(player_id)?;
        let tree_config = self.tree_types.get(&state.tree_type_id)?;

        // Check if tree is still there (might have been chopped by another player)
        if self.is_tree_depleted(state.tree_x, state.tree_y) {
            return None;
        }

        // Calculate swing interval (faster axes = faster swings)
        let effective_multiplier = chop_speed_multiplier.max(0.1);
        let swing_interval_ms = (SWING_INTERVAL_MS as f32 / effective_multiplier) as u64;

        // Check if enough time has elapsed for next swing
        if current_time < state.last_chop_tick + swing_interval_ms {
            return None;
        }

        // Capture values before mutable borrow
        let tree_x = state.tree_x;
        let tree_y = state.tree_y;
        let tree_type_id = state.tree_type_id.clone();
        let log_item_id = tree_config.log_item_id.clone();
        let xp_gained = tree_config.xp_per_log;
        // Effective success = base + tool bonus + 0.5% per level above requirement (capped at 95%)
        let level_bonus = (player_woodcutting_level - tree_config.level_required).max(0) as f32 * 0.005;
        let success_chance = (tree_config.success_chance + chop_success_bonus + level_bonus).min(0.95);
        let depletion_chance = tree_config.depletion_chance;
        let respawn_min = tree_config.respawn_time_min;
        let respawn_max = tree_config.respawn_time_max;
        let tree_gid = tree_config.gids.first().copied().unwrap_or(0);

        // Update last chop tick
        let state_mut = self.player_states.get_mut(player_id)?;
        state_mut.last_chop_tick = current_time;

        // Roll for success (did we get a log?)
        let mut rng = rand::thread_rng();
        let success = rng.r#gen::<f32>() < success_chance;

        if !success {
            // Missed - no log, no XP, no depletion
            return Some(ChopResult {
                success: false,
                log_item_id,
                xp_gained: 0,
                tree_depleted: false,
                respawn_delay_ms: None,
                tree_type_id,
            });
        }

        // Successful chop - roll for tree depletion
        let tree_depleted = rng.r#gen::<f32>() < depletion_chance;

        let respawn_delay_ms = if tree_depleted {
            // Calculate respawn time
            let respawn_delay = rng.gen_range(respawn_min..=respawn_max);
            let respawn_time = current_time + respawn_delay;

            // Mark tree as depleted
            self.depleted_trees.insert(
                (tree_x, tree_y),
                DepletedTree {
                    gid: tree_gid,
                    tree_type_id: tree_type_id.clone(),
                    depleted_at: current_time,
                    respawn_at: respawn_time,
                },
            );

            info!(
                "Tree at ({}, {}) depleted, will respawn in {}ms",
                tree_x, tree_y, respawn_delay
            );
            Some(respawn_delay)
        } else {
            None
        };

        info!(
            "Player {} chopped {} for {} xp (depleted: {})",
            player_id, log_item_id, xp_gained, tree_depleted
        );

        Some(ChopResult {
            success: true,
            log_item_id,
            xp_gained,
            tree_depleted,
            respawn_delay_ms,
            tree_type_id,
        })
    }

    /// Tick respawns and return list of trees that respawned
    pub fn tick_respawns(&mut self, current_time: u64) -> Vec<TreeRespawnEvent> {
        let mut respawned = Vec::new();

        // Find trees that should respawn
        let to_respawn: Vec<(i32, i32)> = self
            .depleted_trees
            .iter()
            .filter(|(_, tree)| current_time >= tree.respawn_at)
            .map(|((x, y), _)| (*x, *y))
            .collect();

        // Respawn them
        for (x, y) in to_respawn {
            if let Some(tree) = self.depleted_trees.remove(&(x, y)) {
                info!("Tree at ({}, {}) respawned", x, y);
                respawned.push(TreeRespawnEvent {
                    x,
                    y,
                    gid: tree.gid,
                });
            }
        }

        respawned
    }

    /// Get all currently depleted trees (for syncing to new clients)
    pub fn get_depleted_trees(&self) -> Vec<((i32, i32), &DepletedTree)> {
        self.depleted_trees
            .iter()
            .map(|(pos, tree)| (*pos, tree))
            .collect()
    }

    /// Deplete a tree externally (used when loading from persistent state if needed)
    pub fn deplete_tree(
        &mut self,
        x: i32,
        y: i32,
        gid: u32,
        tree_type_id: String,
        current_time: u64,
        respawn_at: u64,
    ) {
        self.depleted_trees.insert(
            (x, y),
            DepletedTree {
                gid,
                tree_type_id,
                depleted_at: current_time,
                respawn_at,
            },
        );
    }

    /// Perform a single chop attempt on a tree (player-initiated)
    /// Returns Ok(ChopResult) on success, Err(message) on validation failure
    ///
    /// `tool_success_bonus` - bonus success chance from the equipped axe (0.0 for bronze, up to 0.25 for rune)
    pub fn chop_once(
        &mut self,
        tree_x: i32,
        tree_y: i32,
        tree_gid: u32,
        player_woodcutting_level: i32,
        tool_success_bonus: f32,
        current_time: u64,
    ) -> Result<ChopResult, String> {
        // Check if tree is depleted
        if self.is_tree_depleted(tree_x, tree_y) {
            return Err("This tree has already been chopped down".to_string());
        }

        // Find tree type by GID
        let tree_type_id = self
            .gid_to_tree_type
            .get(&tree_gid)
            .cloned()
            .ok_or_else(|| "Not a valid tree".to_string())?;

        let tree_config = self
            .tree_types
            .get(&tree_type_id)
            .ok_or_else(|| "Unknown tree type".to_string())?;

        // Check level requirement
        if player_woodcutting_level < tree_config.level_required {
            return Err(format!(
                "You need Woodcutting level {} to chop this tree",
                tree_config.level_required
            ));
        }

        // Capture values
        let log_item_id = tree_config.log_item_id.clone();
        let xp_gained = tree_config.xp_per_log;
        // Effective success = base + tool bonus + 0.3% per level above requirement (capped at 40%)
        let level_bonus = (player_woodcutting_level - tree_config.level_required).max(0) as f32 * 0.003;
        let success_chance = (tree_config.success_chance + tool_success_bonus + level_bonus).min(0.40);
        let depletion_chance = tree_config.depletion_chance;
        let respawn_min = tree_config.respawn_time_min;
        let respawn_max = tree_config.respawn_time_max;
        let tree_type_id_for_depletion = tree_type_id.clone();

        // Roll for success
        let mut rng = rand::thread_rng();
        let success = rng.r#gen::<f32>() < success_chance;

        if !success {
            // Miss - swing but no log
            return Ok(ChopResult {
                success: false,
                log_item_id,
                xp_gained: 0,
                tree_depleted: false,
                respawn_delay_ms: None,
                tree_type_id,
            });
        }

        // Success! Roll for depletion
        let tree_depleted = rng.r#gen::<f32>() < depletion_chance;

        let respawn_delay_ms = if tree_depleted {
            let respawn_delay = rng.gen_range(respawn_min..=respawn_max);
            let respawn_time = current_time + respawn_delay;

            self.depleted_trees.insert(
                (tree_x, tree_y),
                DepletedTree {
                    gid: tree_gid,
                    tree_type_id: tree_type_id_for_depletion,
                    depleted_at: current_time,
                    respawn_at: respawn_time,
                },
            );

            info!(
                "Tree at ({}, {}) depleted, respawn in {}ms",
                tree_x, tree_y, respawn_delay
            );
            Some(respawn_delay)
        } else {
            None
        };

        info!(
            "Chop success at ({}, {}): {} (+{}xp, depleted={})",
            tree_x, tree_y, log_item_id, xp_gained, tree_depleted
        );

        Ok(ChopResult {
            success: true,
            log_item_id,
            xp_gained,
            tree_depleted,
            respawn_delay_ms,
            tree_type_id,
        })
    }
}

impl Default for WoodcuttingSystem {
    fn default() -> Self {
        Self::new()
    }
}
