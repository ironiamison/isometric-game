use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct StationDefinition {
    pub display_name: String,
    pub description: String,
    pub interaction_range: u32,
    #[serde(default)]
    pub gids: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct StationRegistry {
    stations: HashMap<String, StationDefinition>,
}

impl StationRegistry {
    pub fn new() -> Self {
        Self {
            stations: HashMap::new(),
        }
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read stations file: {}", e))?;
        let stations: HashMap<String, StationDefinition> = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse stations TOML: {}", e))?;
        self.stations = stations;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&StationDefinition> {
        self.stations.get(id)
    }

    pub fn exists(&self, id: &str) -> bool {
        self.stations.contains_key(id)
    }

    /// Find the station that owns a given map object GID
    pub fn station_for_gid(&self, gid: u32) -> Option<(&str, &StationDefinition)> {
        self.stations
            .iter()
            .find(|(_, def)| def.gids.contains(&gid))
            .map(|(id, def)| (id.as_str(), def))
    }

    /// Get all station GIDs as a map (for sending to client)
    pub fn all_station_gids(&self) -> HashMap<String, Vec<u32>> {
        self.stations
            .iter()
            .map(|(id, def)| (id.clone(), def.gids.clone()))
            .collect()
    }
}
