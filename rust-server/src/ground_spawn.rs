use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

// ============================================================================
// Persistent Ground Item Spawns
// ============================================================================

/// Definition of a persistent ground item spawn (deserialized from TOML).
#[derive(Debug, Clone, Deserialize)]
pub struct GroundSpawnDef {
    pub id: String,
    pub item_id: String,
    pub x: f32,
    pub y: f32,
    pub quantity: i32,
    pub respawn_seconds: u64,
    /// Optional instance/map id (None = overworld)
    #[serde(default)]
    pub instance_id: Option<String>,
}

/// TOML file structure: `[[spawns]]` array-of-tables.
#[derive(Debug, Deserialize)]
struct GroundSpawnsFile {
    #[serde(default)]
    spawns: Vec<GroundSpawnDef>,
}

/// Runtime state for a single persistent ground spawn.
#[derive(Debug)]
pub struct GroundSpawnState {
    pub def: GroundSpawnDef,
    /// None if the item is currently on the ground; Some(when) if it was picked up.
    pub picked_up_at: Option<Instant>,
    /// The id of the currently-active GroundItem (if any).
    pub active_ground_item_id: Option<String>,
}

/// Manages all persistent ground item spawns.
#[derive(Debug)]
pub struct GroundSpawnManager {
    pub spawns: HashMap<String, GroundSpawnState>,
}

impl GroundSpawnManager {
    /// Load spawn definitions from `data_dir/ground_spawns.toml`.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("ground_spawns.toml");
        let spawns = match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<GroundSpawnsFile>(&content) {
                Ok(file) => {
                    tracing::info!(
                        "Loaded {} persistent ground spawn definitions from {:?}",
                        file.spawns.len(),
                        path
                    );
                    file.spawns
                        .into_iter()
                        .map(|def| {
                            let id = def.id.clone();
                            (
                                id,
                                GroundSpawnState {
                                    def,
                                    picked_up_at: None,
                                    active_ground_item_id: None,
                                },
                            )
                        })
                        .collect()
                }
                Err(e) => {
                    tracing::warn!("Failed to parse ground_spawns.toml: {}", e);
                    HashMap::new()
                }
            },
            Err(_) => {
                tracing::info!("No ground_spawns.toml found, skipping persistent spawns");
                HashMap::new()
            }
        };

        Self { spawns }
    }

    /// Mark a ground item as picked up by matching its active_ground_item_id.
    pub fn mark_picked_up(&mut self, ground_item_id: &str) {
        for state in self.spawns.values_mut() {
            if state
                .active_ground_item_id
                .as_deref()
                == Some(ground_item_id)
            {
                state.picked_up_at = Some(Instant::now());
                state.active_ground_item_id = None;
                tracing::debug!(
                    "Persistent ground spawn '{}' picked up, respawning in {}s",
                    state.def.id,
                    state.def.respawn_seconds
                );
                return;
            }
        }
    }

    /// Check for spawns whose respawn timers have elapsed.
    /// Returns a Vec of (spawn_id, item_id, x, y, quantity, instance_id) for items that should respawn.
    pub fn check_respawns(&mut self) -> Vec<(String, String, f32, f32, i32, Option<String>)> {
        let now = Instant::now();
        let mut respawns = Vec::new();

        for state in self.spawns.values_mut() {
            if let Some(picked_up_at) = state.picked_up_at {
                let elapsed = now.duration_since(picked_up_at);
                if elapsed.as_secs() >= state.def.respawn_seconds {
                    state.picked_up_at = None;
                    respawns.push((
                        state.def.id.clone(),
                        state.def.item_id.clone(),
                        state.def.x,
                        state.def.y,
                        state.def.quantity,
                        state.def.instance_id.clone(),
                    ));
                }
            }
        }

        respawns
    }

    /// Record which GroundItem id corresponds to this spawn.
    pub fn set_active_ground_item(&mut self, spawn_id: &str, ground_item_id: String) {
        if let Some(state) = self.spawns.get_mut(spawn_id) {
            state.active_ground_item_id = Some(ground_item_id);
        }
    }

    /// Get all spawns that should be created at startup.
    /// Returns a Vec of (spawn_id, item_id, x, y, quantity, instance_id).
    pub fn get_initial_spawns(&self) -> Vec<(String, String, f32, f32, i32, Option<String>)> {
        self.spawns
            .values()
            .map(|state| {
                (
                    state.def.id.clone(),
                    state.def.item_id.clone(),
                    state.def.x,
                    state.def.y,
                    state.def.quantity,
                    state.def.instance_id.clone(),
                )
            })
            .collect()
    }
}
