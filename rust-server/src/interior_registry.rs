use crate::interior::InteriorMapDef;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

pub struct InteriorRegistry {
    interiors: HashMap<String, InteriorMapDef>,
}

impl InteriorRegistry {
    pub fn load_from_directory(dir: impl AsRef<Path>) -> Result<Self, String> {
        let mut interiors = HashMap::new();
        let path = dir.as_ref();

        if !path.exists() {
            return Err(format!(
                "interiors directory {} does not exist",
                path.display()
            ));
        }

        let entries = std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read interiors directory: {}", e))?;

        for entry in entries {
            let file_path = entry
                .map_err(|error| format!("failed to read interior directory entry: {error}"))?
                .path();
            if file_path.extension().is_some_and(|ext| ext == "json") {
                let interior = InteriorMapDef::load_from_file(&file_path)?;
                info!(
                    "Loaded interior map: {} ({}) with {} entities",
                    interior.id,
                    interior.name,
                    interior.entities.len()
                );
                if interiors.insert(interior.id.clone(), interior).is_some() {
                    return Err(format!("duplicate interior id in {}", file_path.display()));
                }
            }
        }

        if interiors.is_empty() {
            return Err(format!(
                "interiors directory {} contains no JSON maps",
                path.display()
            ));
        }
        info!("Loaded {} interior maps", interiors.len());
        Ok(Self { interiors })
    }

    pub fn get(&self, id: &str) -> Option<&InteriorMapDef> {
        self.interiors.get(id)
    }

    pub fn list_ids(&self) -> Vec<&String> {
        self.interiors.keys().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &InteriorMapDef)> {
        self.interiors.iter()
    }
}
