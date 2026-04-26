use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionLogDefinitions {
    #[serde(default)]
    pub monster_drops: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub boss_rewards: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub skilling: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub quest_rewards: HashMap<String, Vec<String>>,
}

impl CollectionLogDefinitions {
    pub fn load(path: &str) -> Self {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e))
    }

    /// Get all (item_id, source, source_detail) triples for protocol transmission
    pub fn all_entries(&self) -> Vec<(String, String, String)> {
        let mut entries = Vec::new();
        for (monster, items) in &self.monster_drops {
            for item in items {
                entries.push((item.clone(), "monster_drops".to_string(), monster.clone()));
            }
        }
        for (boss, items) in &self.boss_rewards {
            for item in items {
                entries.push((item.clone(), "boss_rewards".to_string(), boss.clone()));
            }
        }
        for (skill, items) in &self.skilling {
            for item in items {
                entries.push((item.clone(), "skilling".to_string(), skill.clone()));
            }
        }
        for (quest, items) in &self.quest_rewards {
            for item in items {
                entries.push((item.clone(), "quest_rewards".to_string(), quest.clone()));
            }
        }
        entries
    }
}
