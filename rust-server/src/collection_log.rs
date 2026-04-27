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

    /// Build a map of source_detail_id -> display_name using entity registry.
    /// Quest names must be provided separately (since QuestRegistry is async).
    pub fn build_display_names(
        &self,
        entity_registry: &crate::entity::EntityRegistry,
        quest_names: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut names = HashMap::new();

        // Monster and boss display names from entity prototypes
        for id in self.monster_drops.keys().chain(self.boss_rewards.keys()) {
            if let Some(proto) = entity_registry.get(id) {
                names.insert(id.clone(), proto.display_name.clone());
            }
        }

        // Skill display names
        for id in self.skilling.keys() {
            let display = match id.as_str() {
                "fishing" => "Fishing",
                "mining" => "Mining",
                "woodcutting" => "Woodcutting",
                "cooking" => "Cooking",
                "smithing" => "Smithing",
                "alchemy" => "Alchemy",
                "farming" => "Farming",
                "fletching" => "Fletching",
                "leatherworking" => "Leatherworking",
                _ => "",
            };
            if !display.is_empty() {
                names.insert(id.clone(), display.to_string());
            }
        }

        // Quest display names
        for id in self.quest_rewards.keys() {
            if let Some(name) = quest_names.get(id) {
                names.insert(id.clone(), name.clone());
            }
        }

        names
    }
}
