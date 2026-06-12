use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Type of instance this interior creates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceType {
    Public,
    Private,
}

/// A spawn point inside an interior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub x: f32,
    pub y: f32,
}

/// A portal/exit inside an interior that leads elsewhere
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteriorPortal {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub target_map: String,
    pub target_x: f32,
    pub target_y: f32,
    pub target_spawn: Option<String>,
}

/// A map object placed from object layer (trees, rocks, decorations)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteriorMapObject {
    pub gid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A wall placed on a tile edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteriorWall {
    pub gid: u32,
    pub x: i32,
    pub y: i32,
    pub edge: String,
}

/// Entity spawn point in an interior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteriorEntitySpawn {
    pub entity_id: String,
    pub x: i32,
    pub y: i32,
    /// Level override (None = use prototype's level)
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub unique_id: Option<String>,
    #[serde(default)]
    pub facing: Option<String>,
    #[serde(default = "default_true")]
    pub respawn: bool,
}
fn default_true() -> bool {
    true
}

/// A gathering zone marker inside an interior (fishing spot)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteriorGatheringZone {
    pub x: i32,
    pub y: i32,
    pub zone_id: String,
}

/// A chest spawn point inside an interior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteriorChestSpawn {
    pub chest_id: String,
    pub x: i32,
    pub y: i32,
}

/// Definition of an interior map (loaded from JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteriorMapDef {
    pub id: String,
    pub name: String,
    pub instance_type: InstanceType,
    pub size: InteriorSize,
    pub spawn_points: HashMap<String, SpawnPoint>,
    pub portals: Vec<InteriorPortal>,
    // Map data fields
    #[serde(default)]
    pub layers: InteriorLayers,
    #[serde(default)]
    pub collision: String, // Base64 encoded or empty
    #[serde(default)]
    pub entities: Vec<InteriorEntitySpawn>,
    #[serde(default, rename = "mapObjects")]
    pub map_objects: Vec<InteriorMapObject>,
    #[serde(default)]
    pub walls: Vec<InteriorWall>,
    #[serde(default)]
    pub requires_slayer_task: bool,
    #[serde(default)]
    pub chests: Vec<InteriorChestSpawn>,
    #[serde(default, rename = "gatheringZones")]
    pub gathering_zones: Vec<InteriorGatheringZone>,
    /// Optional heightmap for interiors with elevation (e.g. KOTH arena)
    #[serde(default)]
    pub heightmap: Option<Vec<u8>>,
    /// Optional block types for downward tile edges (visual)
    #[serde(default, rename = "blockTypesDown")]
    pub block_types_down: Option<Vec<u16>>,
    /// Optional block types for rightward tile edges (visual)
    #[serde(default, rename = "blockTypesRight")]
    pub block_types_right: Option<Vec<u16>>,
    /// Whether PVP is enabled in this instance (default: false)
    #[serde(default, rename = "pvpEnabled")]
    pub pvp_enabled: bool,
}

/// Layer data for interior maps
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InteriorLayers {
    #[serde(default)]
    pub ground: Vec<u32>,
    #[serde(default)]
    pub objects: Vec<u32>,
    #[serde(default)]
    pub overhead: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteriorSize {
    pub width: u32,
    pub height: u32,
}

impl InteriorMapDef {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("Failed to read interior file {}: {error}", path.display()))?;
        let interior: Self = serde_json::from_str(&content).map_err(|error| {
            format!("Failed to parse interior JSON {}: {error}", path.display())
        })?;
        interior
            .validate()
            .map_err(|error| format!("Invalid interior {}: {error}", path.display()))?;
        Ok(interior)
    }

    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
        {
            return Err(
                "id must contain only ASCII letters, digits, underscores, or dashes".into(),
            );
        }
        if self.name.trim().is_empty() {
            return Err("name cannot be empty".into());
        }
        if self.size.width == 0
            || self.size.height == 0
            || self.size.width > 512
            || self.size.height > 512
        {
            return Err("dimensions must be between 1 and 512".into());
        }
        let tile_count = usize::try_from(self.size.width * self.size.height)
            .map_err(|_| "tile count overflow")?;
        for (name, layer) in [
            ("ground", &self.layers.ground),
            ("objects", &self.layers.objects),
            ("overhead", &self.layers.overhead),
        ] {
            if layer.len() != tile_count {
                return Err(format!(
                    "{name} layer has {} tiles; expected {tile_count}",
                    layer.len()
                ));
            }
        }
        if !self.collision.is_empty() {
            let collision = base64::engine::general_purpose::STANDARD
                .decode(&self.collision)
                .map_err(|error| format!("collision is not valid base64: {error}"))?;
            let expected = tile_count.div_ceil(8);
            if collision.len() != expected {
                return Err(format!(
                    "collision has {} bytes; expected {expected}",
                    collision.len()
                ));
            }
        }
        if let Some(heightmap) = &self.heightmap
            && heightmap.len() != tile_count
        {
            return Err(format!(
                "heightmap has {} entries; expected {tile_count}",
                heightmap.len()
            ));
        }
        for (name, blocks) in [
            ("blockTypesDown", &self.block_types_down),
            ("blockTypesRight", &self.block_types_right),
        ] {
            if let Some(blocks) = blocks
                && blocks.len() != tile_count
            {
                return Err(format!(
                    "{name} has {} entries; expected {tile_count}",
                    blocks.len()
                ));
            }
        }
        for (name, spawn) in &self.spawn_points {
            if name.trim().is_empty() || !self.contains_position(spawn.x, spawn.y) {
                return Err(format!("spawn point '{name}' is empty or outside the map"));
            }
        }
        for portal in &self.portals {
            if portal.id.is_empty()
                || portal.target_map.is_empty()
                || portal.width <= 0
                || portal.height <= 0
                || portal.x < 0
                || portal.y < 0
                || portal.x + portal.width > self.size.width as i32
                || portal.y + portal.height > self.size.height as i32
            {
                return Err(format!("invalid portal '{}'", portal.id));
            }
        }
        for entity in &self.entities {
            if entity.entity_id.is_empty()
                || entity.x < 0
                || entity.y < 0
                || entity.x >= self.size.width as i32
                || entity.y >= self.size.height as i32
                || entity.level.is_some_and(|level| level <= 0)
            {
                return Err(format!("invalid entity spawn '{}'", entity.entity_id));
            }
        }
        let mut unique_entity_ids = std::collections::HashSet::new();
        for entity in &self.entities {
            if let Some(unique_id) = entity.unique_id.as_deref()
                && !unique_id.is_empty()
                && !unique_entity_ids.insert(unique_id)
            {
                return Err(format!("duplicate entity uniqueId '{unique_id}'"));
            }
        }
        for wall in &self.walls {
            if !matches!(wall.edge.as_str(), "down" | "right")
                || wall.x < 0
                || wall.y < 0
                || wall.x >= self.size.width as i32
                || wall.y >= self.size.height as i32
            {
                return Err(format!("invalid wall at ({}, {})", wall.x, wall.y));
            }
        }
        Ok(())
    }

    fn contains_position(&self, x: f32, y: f32) -> bool {
        x.is_finite()
            && y.is_finite()
            && x >= 0.0
            && y >= 0.0
            && x < self.size.width as f32
            && y < self.size.height as f32
    }

    pub fn get_spawn_point(&self, name: &str) -> Option<&SpawnPoint> {
        self.spawn_points.get(name)
    }

    pub fn get_portal_at(&self, x: i32, y: i32) -> Option<&InteriorPortal> {
        self.portals
            .iter()
            .find(|p| x >= p.x && x < p.x + p.width && y >= p.y && y < p.y + p.height)
    }
}

pub use crate::interior_registry::InteriorRegistry;
