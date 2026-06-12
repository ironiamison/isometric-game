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
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("waystones.toml");
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let file: WaystoneFile = toml::from_str(&source)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

        let mut waystones = HashMap::new();
        let mut by_position = HashMap::new();

        for def in file.waystones {
            if def.id.is_empty() || def.name.is_empty() || def.linked_to.is_empty() {
                return Err("waystone id, name, and linked_to must be non-empty".to_string());
            }
            if by_position.insert((def.x, def.y), def.id.clone()).is_some() {
                return Err(format!(
                    "duplicate waystone position ({}, {})",
                    def.x, def.y
                ));
            }
            let id = def.id.clone();
            if waystones.insert(id.clone(), def).is_some() {
                return Err(format!("duplicate waystone id '{id}'"));
            }
        }
        for waystone in waystones.values() {
            if !waystones.contains_key(&waystone.linked_to) {
                return Err(format!(
                    "waystone '{}' links to unknown waystone '{}'",
                    waystone.id, waystone.linked_to
                ));
            }
        }

        tracing::info!("Loaded {} waystone(s) from {:?}", waystones.len(), path);
        Ok(Self {
            waystones,
            by_position,
        })
    }

    pub async fn validate_quests(
        &self,
        quests: &crate::quest::QuestRegistry,
    ) -> Result<(), String> {
        for waystone in self.waystones.values() {
            if quests.get(&waystone.quest_required).await.is_none() {
                return Err(format!(
                    "waystone '{}' references unknown quest '{}'",
                    waystone.id, waystone.quest_required
                ));
            }
        }
        Ok(())
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

    pub fn iter(&self) -> impl Iterator<Item = &WaystoneDef> {
        self.waystones.values()
    }
}
