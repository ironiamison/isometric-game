//! Mining skill system.
//!
//! Allows players to mine rocks on the map. Rocks are identified by their GID
//! from the tileset. When depleted, rocks disappear and respawn after a delay.

use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

// ---------------------------------------------------------------------------
// TOML deserialization structures
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
pub struct OreTypeConfig {
    pub gids: Vec<u32>,
    pub level_required: i32,
    pub ore_item_id: String,
    pub xp_per_ore: i64,
    /// Chance to successfully get an ore on each swing (0.0 - 1.0)
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
struct OreTypesFile {
    rocks: HashMap<String, OreTypeConfig>,
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DepletedRock {
    pub gid: u32,
    pub ore_type_id: String,
    pub depleted_at: u64,
    pub respawn_at: u64,
}

#[derive(Debug, Clone)]
pub struct PlayerMiningState {
    pub rock_x: i32,
    pub rock_y: i32,
    pub ore_type_id: String,
    pub last_mine_tick: u64,
}

/// Swing interval in milliseconds (how often the player swings their pickaxe)
/// Base swing interval - matches client attack cooldown (800ms)
pub const SWING_INTERVAL_MS: u64 = 700;

#[derive(Debug, Clone)]
pub struct MineResult {
    /// Whether this swing successfully got an ore
    pub success: bool,
    /// Item ID of the ore (only meaningful if success=true)
    pub ore_item_id: String,
    /// XP gained (only meaningful if success=true)
    pub xp_gained: i64,
    /// Whether the rock was depleted (only on successful mines)
    pub rock_depleted: bool,
    pub respawn_delay_ms: Option<u64>,
    /// Ore type ID (e.g., "bronze", "iron") - for quest tracking
    pub ore_type_id: String,
    /// Bonus gem drop (if any) - item ID of the uncut gem
    pub gem_drop: Option<String>,
}

/// Gem tiers with drop rates (increasingly rare for higher-level gems).
/// Checked on every successful ore mine.
const GEM_DROP_TABLE: &[(&str, f64)] = &[
    ("uncut_sapphire", 1.0 / 64.0), // ~1.56%
    ("uncut_emerald", 1.0 / 128.0), // ~0.78%
    ("uncut_ruby", 1.0 / 256.0),    // ~0.39%
    ("uncut_diamond", 1.0 / 512.0), // ~0.20%
];

/// Roll for a bonus gem drop. Returns the item ID if one drops.
fn roll_gem_drop(rng: &mut impl Rng) -> Option<String> {
    let roll: f64 = rng.r#gen();
    let mut cumulative = 0.0;
    for &(gem_id, chance) in GEM_DROP_TABLE {
        cumulative += chance;
        if roll < cumulative {
            return Some(gem_id.to_string());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct RockRespawnEvent {
    pub x: i32,
    pub y: i32,
    pub gid: u32,
}

// ---------------------------------------------------------------------------
// MiningSystem
// ---------------------------------------------------------------------------

pub struct MiningSystem {
    /// Ore type ID -> config
    pub ore_types: HashMap<String, OreTypeConfig>,
    /// GID -> ore type ID mapping for fast lookup
    pub gid_to_ore_type: HashMap<u32, String>,
    /// (instance_id, x, y) -> depleted rock state (None = overworld)
    pub depleted_rocks: HashMap<(Option<String>, i32, i32), DepletedRock>,
    /// Player ID -> mining state
    pub player_states: HashMap<String, PlayerMiningState>,
}

impl MiningSystem {
    pub fn new() -> Self {
        Self {
            ore_types: HashMap::new(),
            gid_to_ore_type: HashMap::new(),
            depleted_rocks: HashMap::new(),
            player_states: HashMap::new(),
        }
    }

    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("ore_types.toml");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read ore_types.toml: {}", e))?;

        let file: OreTypesFile = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse ore_types.toml: {}", e))?;

        let mut gid_to_ore_type = HashMap::new();
        for (type_id, config) in &file.rocks {
            for gid in &config.gids {
                gid_to_ore_type.insert(*gid, type_id.clone());
            }
        }

        info!(
            "Loaded {} ore types with {} GID mappings",
            file.rocks.len(),
            gid_to_ore_type.len()
        );

        Ok(Self {
            ore_types: file.rocks,
            gid_to_ore_type,
            depleted_rocks: HashMap::new(),
            player_states: HashMap::new(),
        })
    }

    /// Check if a GID is a rock that can be mined
    pub fn is_rock_gid(&self, gid: u32) -> bool {
        self.gid_to_ore_type.contains_key(&gid)
    }

    /// Get the ore type config for a GID
    pub fn get_ore_type(&self, gid: u32) -> Option<&OreTypeConfig> {
        let type_id = self.gid_to_ore_type.get(&gid)?;
        self.ore_types.get(type_id)
    }

    /// Get ore type ID for a GID
    pub fn get_ore_type_id(&self, gid: u32) -> Option<&String> {
        self.gid_to_ore_type.get(&gid)
    }

    /// Check if a rock at given position is depleted
    pub fn is_rock_depleted(&self, instance_id: Option<&str>, x: i32, y: i32) -> bool {
        self.depleted_rocks.contains_key(&(instance_id.map(|s| s.to_string()), x, y))
    }

    /// Tick respawns and return list of rocks that respawned
    pub fn tick_respawns(&mut self, current_time: u64) -> Vec<RockRespawnEvent> {
        let mut respawned = Vec::new();

        // Find rocks that should respawn
        let to_respawn: Vec<(Option<String>, i32, i32)> = self
            .depleted_rocks
            .iter()
            .filter(|(_, rock)| current_time >= rock.respawn_at)
            .map(|((inst, x, y), _)| (inst.clone(), *x, *y))
            .collect();

        // Respawn them
        for key @ (_, x, y) in to_respawn {
            if let Some(rock) = self.depleted_rocks.remove(&key) {
                info!("Rock at ({}, {}) respawned", x, y);
                respawned.push(RockRespawnEvent {
                    x,
                    y,
                    gid: rock.gid,
                });
            }
        }

        respawned
    }

    /// Get all currently depleted rocks (for syncing to new clients)
    pub fn get_depleted_rocks(&self, instance_id: Option<&str>) -> Vec<((i32, i32), &DepletedRock)> {
        self.depleted_rocks
            .iter()
            .filter(|((inst, _, _), _)| inst.as_deref() == instance_id)
            .map(|((_, x, y), rock)| ((*x, *y), rock))
            .collect()
    }

    /// Deplete a rock externally (used when loading from persistent state if needed)
    pub fn deplete_rock(
        &mut self,
        instance_id: Option<String>,
        x: i32,
        y: i32,
        gid: u32,
        ore_type_id: String,
        current_time: u64,
        respawn_at: u64,
    ) {
        self.depleted_rocks.insert(
            (instance_id, x, y),
            DepletedRock {
                gid,
                ore_type_id,
                depleted_at: current_time,
                respawn_at,
            },
        );
    }

    /// Perform a single mine attempt on a rock (player-initiated)
    /// Returns Ok(MineResult) on success, Err(message) on validation failure
    ///
    /// `tool_success_bonus` - bonus success chance from the equipped pickaxe (0.0 for bronze, up to 0.25 for rune)
    pub fn mine_once(
        &mut self,
        instance_id: Option<&str>,
        rock_x: i32,
        rock_y: i32,
        rock_gid: u32,
        player_mining_level: i32,
        tool_success_bonus: f32,
        current_time: u64,
    ) -> Result<MineResult, String> {
        // Check if rock is depleted
        if self.is_rock_depleted(instance_id, rock_x, rock_y) {
            return Err("This rock has already been mined".to_string());
        }

        // Find ore type by GID
        let ore_type_id = self
            .gid_to_ore_type
            .get(&rock_gid)
            .cloned()
            .ok_or_else(|| "Not a valid rock".to_string())?;

        let ore_config = self
            .ore_types
            .get(&ore_type_id)
            .ok_or_else(|| "Unknown ore type".to_string())?;

        // Check level requirement
        if player_mining_level < ore_config.level_required {
            return Err(format!(
                "You need Mining level {} to mine this rock",
                ore_config.level_required
            ));
        }

        // Capture values
        let ore_item_id = ore_config.ore_item_id.clone();
        let xp_gained = ore_config.xp_per_ore;
        // Effective success = base + tool bonus + 0.3% per level above requirement (capped at 40%)
        let level_bonus = (player_mining_level - ore_config.level_required).max(0) as f32 * 0.003;
        let success_chance =
            (ore_config.success_chance + tool_success_bonus + level_bonus).min(0.40);
        let depletion_chance = ore_config.depletion_chance;
        let respawn_min = ore_config.respawn_time_min;
        let respawn_max = ore_config.respawn_time_max;
        let ore_type_id_for_depletion = ore_type_id.clone();

        // Roll for success
        let mut rng = rand::thread_rng();
        let success = rng.r#gen::<f32>() < success_chance;

        if !success {
            // Miss - swing but no ore
            return Ok(MineResult {
                success: false,
                ore_item_id,
                xp_gained: 0,
                rock_depleted: false,
                respawn_delay_ms: None,
                ore_type_id,
                gem_drop: None,
            });
        }

        // Success! Roll for depletion
        let rock_depleted = rng.r#gen::<f32>() < depletion_chance;

        let respawn_delay_ms = if rock_depleted {
            let respawn_delay = rng.gen_range(respawn_min..=respawn_max);
            let respawn_time = current_time + respawn_delay;

            self.depleted_rocks.insert(
                (instance_id.map(|s| s.to_string()), rock_x, rock_y),
                DepletedRock {
                    gid: rock_gid,
                    ore_type_id: ore_type_id_for_depletion,
                    depleted_at: current_time,
                    respawn_at: respawn_time,
                },
            );

            info!(
                "Rock at ({}, {}) depleted, respawn in {}ms",
                rock_x, rock_y, respawn_delay
            );
            Some(respawn_delay)
        } else {
            None
        };

        // Roll for bonus gem drop
        let gem_drop = roll_gem_drop(&mut rng);
        if let Some(ref gem) = gem_drop {
            info!(
                "Gem drop at ({}, {}): {} while mining {}",
                rock_x, rock_y, gem, ore_item_id
            );
        }

        info!(
            "Mine success at ({}, {}): {} (+{}xp, depleted={})",
            rock_x, rock_y, ore_item_id, xp_gained, rock_depleted
        );

        Ok(MineResult {
            success: true,
            ore_item_id,
            xp_gained,
            rock_depleted,
            respawn_delay_ms,
            ore_type_id,
            gem_drop,
        })
    }
}

impl Default for MiningSystem {
    fn default() -> Self {
        Self::new()
    }
}
