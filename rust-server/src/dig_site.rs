//! Dig Sites System
//!
//! Loads dig site definitions from TOML and provides lookup/trigger tracking.
//! Dig sites spawn entities when a player uses a shovel near a specific location
//! while meeting quest requirements.

use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

/// A single dig site definition (deserialized from TOML)
#[derive(Debug, Clone, Deserialize)]
pub struct DigSiteDef {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub quest_id: String,
    pub quest_objective_id: String,
    pub spawn_entity: String,
    pub spawn_level: i32,
}

/// TOML wrapper for array of sites
#[derive(Debug, Deserialize)]
struct DigSiteFile {
    sites: Vec<DigSiteDef>,
}

/// Manages dig site definitions and tracks which sites have been triggered per player
pub struct DigSiteManager {
    pub sites: Vec<DigSiteDef>,
    /// Set of (player_id, site_id) pairs that have already been triggered
    pub triggered: HashSet<(String, String)>,
}

impl DigSiteManager {
    /// Load dig site definitions from `data_dir/dig_sites.toml`
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("dig_sites.toml");
        let sites = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<DigSiteFile>(&contents) {
                    Ok(file) => {
                        tracing::info!("Loaded {} dig site(s) from {:?}", file.sites.len(), path);
                        file.sites
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse dig_sites.toml: {}", e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read dig_sites.toml: {}", e);
                    Vec::new()
                }
            }
        } else {
            tracing::warn!("No dig_sites.toml found at {:?}, no dig sites loaded", path);
            Vec::new()
        };

        Self {
            sites,
            triggered: HashSet::new(),
        }
    }

    /// Mark a (player, site) pair as triggered so it won't fire again
    pub fn mark_triggered(&mut self, player_id: &str, site_id: &str) {
        self.triggered
            .insert((player_id.to_string(), site_id.to_string()));
    }
}
