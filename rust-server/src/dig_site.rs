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
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("dig_sites.toml");
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let file: DigSiteFile = toml::from_str(&source)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let mut ids = HashSet::new();
        for site in &file.sites {
            if site.id.is_empty()
                || site.quest_id.is_empty()
                || site.quest_objective_id.is_empty()
                || site.spawn_entity.is_empty()
                || site.radius < 0
                || site.spawn_level <= 0
            {
                return Err(format!("invalid dig site definition '{}'", site.id));
            }
            if !ids.insert(site.id.clone()) {
                return Err(format!("duplicate dig site id '{}'", site.id));
            }
        }
        tracing::info!("Loaded {} dig site(s) from {:?}", file.sites.len(), path);

        Ok(Self {
            sites: file.sites,
            triggered: HashSet::new(),
        })
    }

    pub async fn validate_references(
        &self,
        entities: &crate::entity::EntityRegistry,
        quests: &crate::quest::QuestRegistry,
    ) -> Result<(), String> {
        for site in &self.sites {
            if entities.get(&site.spawn_entity).is_none() {
                return Err(format!(
                    "dig site '{}' references unknown entity '{}'",
                    site.id, site.spawn_entity
                ));
            }
            let quest = quests.get(&site.quest_id).await.ok_or_else(|| {
                format!(
                    "dig site '{}' references unknown quest '{}'",
                    site.id, site.quest_id
                )
            })?;
            if !quest
                .objectives
                .iter()
                .any(|objective| objective.id == site.quest_objective_id)
            {
                return Err(format!(
                    "dig site '{}' references unknown objective '{}' in quest '{}'",
                    site.id, site.quest_objective_id, site.quest_id
                ));
            }
        }
        Ok(())
    }

    /// Mark a (player, site) pair as triggered so it won't fire again
    pub fn mark_triggered(&mut self, player_id: &str, site_id: &str) {
        self.triggered
            .insert((player_id.to_string(), site_id.to_string()));
    }
}
