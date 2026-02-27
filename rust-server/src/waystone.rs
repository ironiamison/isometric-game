//! Waystone Fast-Travel System
//!
//! Loads waystone definitions from TOML and provides spatial lookup for teleportation.
//! Waystones are paired obelisks that allow players to fast-travel between them
//! once a prerequisite quest has been completed.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A single waystone definition (deserialized from TOML)
#[derive(Debug, Clone, Deserialize)]
pub struct WaystoneDef {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub linked_to: String,
    pub quest_required: String,
}

/// TOML wrapper for array of waystones
#[derive(Debug, Deserialize)]
struct WaystoneFile {
    waystones: Vec<WaystoneDef>,
}

/// Manages waystone definitions and provides spatial + id lookup
pub struct WaystoneManager {
    /// All waystones keyed by their id
    waystones: HashMap<String, WaystoneDef>,
    /// Spatial lookup: (x, y) -> waystone id
    by_position: HashMap<(i32, i32), String>,
}

impl WaystoneManager {
    /// Load waystone definitions from `data_dir/waystones.toml`
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("waystones.toml");
        let defs = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<WaystoneFile>(&contents) {
                    Ok(file) => {
                        tracing::info!(
                            "Loaded {} waystone(s) from {:?}",
                            file.waystones.len(),
                            path
                        );
                        file.waystones
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse waystones.toml: {}", e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read waystones.toml: {}", e);
                    Vec::new()
                }
            }
        } else {
            tracing::warn!("No waystones.toml found at {:?}, no waystones loaded", path);
            Vec::new()
        };

        let mut waystones = HashMap::new();
        let mut by_position = HashMap::new();

        for def in defs {
            by_position.insert((def.x, def.y), def.id.clone());
            waystones.insert(def.id.clone(), def);
        }

        Self {
            waystones,
            by_position,
        }
    }

    /// Find a waystone near position (checks exact tile and +-1 for click tolerance)
    pub fn get_at(&self, x: i32, y: i32) -> Option<&WaystoneDef> {
        // Check exact position first
        if let Some(id) = self.by_position.get(&(x, y)) {
            return self.waystones.get(id);
        }
        // Check +-1 tile for click tolerance
        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(id) = self.by_position.get(&(x + dx, y + dy)) {
                    return self.waystones.get(id);
                }
            }
        }
        None
    }

    /// Get the destination waystone linked to the given waystone id
    pub fn get_destination(&self, waystone_id: &str) -> Option<&WaystoneDef> {
        self.waystones
            .get(waystone_id)
            .and_then(|ws| self.waystones.get(&ws.linked_to))
    }
}
