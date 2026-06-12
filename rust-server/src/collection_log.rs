use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        toml::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))
    }

    pub fn validate(
        &self,
        item_registry: &crate::data::ItemRegistry,
        entity_registry: &crate::entity::EntityRegistry,
        quest_names: &HashMap<String, String>,
    ) -> Result<(), String> {
        const SKILL_SOURCES: &[&str] = &[
            "alchemy",
            "cooking",
            "farming",
            "fishing",
            "fletching",
            "leatherworking",
            "mining",
            "smithing",
            "woodcutting",
        ];

        for source in self.monster_drops.keys().chain(self.boss_rewards.keys()) {
            if !entity_registry.contains(source) {
                return Err(format!("unknown entity source '{source}'"));
            }
        }
        for source in self.quest_rewards.keys() {
            if !quest_names.contains_key(source) {
                return Err(format!("unknown quest source '{source}'"));
            }
        }
        for source in self.skilling.keys() {
            if !SKILL_SOURCES.contains(&source.as_str()) {
                return Err(format!("unknown skilling source '{source}'"));
            }
        }

        for (item_id, source, detail) in self.all_entries() {
            if item_registry.get(&item_id).is_none() {
                return Err(format!(
                    "{source} source '{detail}' references unknown item '{item_id}'"
                ));
            }
        }

        for (category, sources) in [
            ("monster_drops", &self.monster_drops),
            ("boss_rewards", &self.boss_rewards),
            ("skilling", &self.skilling),
            ("quest_rewards", &self.quest_rewards),
        ] {
            for (source, items) in sources {
                if items.is_empty() {
                    return Err(format!("{category} source '{source}' has no items"));
                }
                let unique = items.iter().collect::<HashSet<_>>();
                if unique.len() != items.len() {
                    return Err(format!(
                        "{category} source '{source}' contains duplicate items"
                    ));
                }
            }
        }

        Ok(())
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
                _ => unreachable!("collection log skill source was validated"),
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
