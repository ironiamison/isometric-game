use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct StationDefinition {
    pub display_name: String,
    pub description: String,
    pub interaction_range: u32,
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
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read stations file: {}", e))?;
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
}
