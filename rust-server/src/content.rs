use std::path::Path;
use std::sync::Arc;

use crate::chest::ChestRegistry;
use crate::collection_log::CollectionLogDefinitions;
use crate::crafting::CraftingRegistry;
use crate::data::ItemRegistry;
use crate::entity::EntityRegistry;
use crate::interior_registry::InteriorRegistry;
use crate::prayer::PrayerRegistry;
use crate::quest::QuestRegistry;

/// Immutable, validated content shared by HTTP handlers and every game room.
pub(super) struct ContentRegistries {
    pub entity_registry: Arc<EntityRegistry>,
    pub item_registry: Arc<ItemRegistry>,
    pub prayer_registry: Arc<PrayerRegistry>,
    pub quest_registry: Arc<QuestRegistry>,
    pub crafting_registry: Arc<CraftingRegistry>,
    pub chest_registry: Arc<ChestRegistry>,
    pub interior_registry: Arc<InteriorRegistry>,
    pub collection_log_defs: Arc<CollectionLogDefinitions>,
    pub collection_log_display_names: Arc<Vec<(String, String)>>,
}

impl ContentRegistries {
    pub async fn load(data_dir: &Path, maps_dir: &Path) -> Result<Self, String> {
        validate_content_files(data_dir, maps_dir)?;

        let mut entity_registry = EntityRegistry::new();
        entity_registry
            .load_from_directory(data_dir)
            .map_err(|error| format!("entity registry: {error}"))?;
        if entity_registry.is_empty() {
            return Err("entity registry is empty".to_string());
        }

        let mut item_registry = ItemRegistry::new();
        item_registry
            .load_from_directory(data_dir)
            .map_err(|error| format!("item registry: {error}"))?;
        if item_registry.is_empty() {
            return Err("item registry is empty".to_string());
        }
        entity_registry
            .validate_items(&item_registry)
            .map_err(|error| format!("entity registry: {error}"))?;

        let mut prayer_registry = PrayerRegistry::new();
        prayer_registry
            .load_from_directory(data_dir)
            .map_err(|error| format!("prayer registry: {error}"))?;
        if prayer_registry.is_empty() {
            return Err("prayer registry is empty".to_string());
        }

        let quest_registry = Arc::new(QuestRegistry::new(data_dir));
        quest_registry
            .load_all()
            .await
            .map_err(|error| format!("quest registry: {error}"))?;
        if quest_registry.count().await == 0 {
            return Err("quest registry is empty".to_string());
        }

        let mut crafting_registry = CraftingRegistry::new();
        crafting_registry
            .load_from_directory(data_dir)
            .map_err(|error| format!("crafting registry: {error}"))?;
        if crafting_registry.is_empty() {
            return Err("crafting registry is empty".to_string());
        }
        crafting_registry
            .validate_items(&item_registry)
            .map_err(|error| format!("crafting registry: {error}"))?;

        let mut chest_registry = ChestRegistry::new();
        chest_registry
            .load_from_file(&data_dir.join("chests.toml"))
            .map_err(|error| format!("chest registry: {error}"))?;
        if chest_registry.is_empty() {
            return Err("chest registry is empty".to_string());
        }
        chest_registry
            .validate_items(&item_registry)
            .map_err(|error| format!("chest registry: {error}"))?;

        let interior_registry = InteriorRegistry::load_from_directory(maps_dir.join("interiors"))
            .map_err(|error| format!("interior registry: {error}"))?;
        for (interior_id, interior) in interior_registry.iter() {
            for entity in &interior.entities {
                if entity_registry.get(&entity.entity_id).is_none() {
                    return Err(format!(
                        "interior '{interior_id}' references unknown entity '{}'",
                        entity.entity_id
                    ));
                }
            }
            for chest in &interior.chests {
                if chest_registry.get(&chest.chest_id).is_none() {
                    return Err(format!(
                        "interior '{interior_id}' references unknown chest '{}'",
                        chest.chest_id
                    ));
                }
            }
        }

        let quest_names = quest_registry
            .all_quests()
            .await
            .into_iter()
            .map(|quest| (quest.id.clone(), quest.name.clone()))
            .collect();
        let collection_log_defs =
            CollectionLogDefinitions::load(&data_dir.join("collection_log.toml"))
                .map_err(|error| format!("collection log: {error}"))?;
        collection_log_defs
            .validate(&item_registry, &entity_registry, &quest_names)
            .map_err(|error| format!("collection log: {error}"))?;
        let collection_log_display_names = collection_log_defs
            .build_display_names(&entity_registry, &quest_names)
            .into_iter()
            .collect();

        Ok(Self {
            entity_registry: Arc::new(entity_registry),
            item_registry: Arc::new(item_registry),
            prayer_registry: Arc::new(prayer_registry),
            quest_registry,
            crafting_registry: Arc::new(crafting_registry),
            chest_registry: Arc::new(chest_registry),
            interior_registry: Arc::new(interior_registry),
            collection_log_defs: Arc::new(collection_log_defs),
            collection_log_display_names: Arc::new(collection_log_display_names),
        })
    }
}

fn validate_content_files(data_dir: &Path, maps_dir: &Path) -> Result<(), String> {
    validate_tree(data_dir, "toml", |path, source| {
        toml::from_str::<toml::Value>(source)
            .map(|_| ())
            .map_err(|error| format!("invalid TOML {}: {error}", path.display()))
    })?;
    validate_tree(maps_dir, "json", |path, source| {
        serde_json::from_str::<serde_json::Value>(source)
            .map(|_| ())
            .map_err(|error| format!("invalid JSON {}: {error}", path.display()))
    })
}

fn validate_tree(
    directory: &Path,
    extension: &str,
    validate: fn(&Path, &str) -> Result<(), String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| {
                format!(
                    "failed to read directory entry in {}: {error}",
                    directory.display()
                )
            })?
            .path();
        if path.is_dir() {
            validate_tree(&path, extension, validate)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            let source = std::fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            validate(&path, &source)?;
        }
    }
    Ok(())
}
