use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::chunk::{
    CHUNK_SIZE, Chunk, ChunkCoord, ChunkLayer, ChunkLayerType, EntitySpawn, GatheringZoneMarker,
    MapObject, Portal, Wall, WallEdge, world_to_local,
};

const CHUNK_TILE_COUNT: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

#[derive(Deserialize)]
struct CollisionIgnoreConfig {
    objects_first_gid: u32,
    ignore_object_ids: Vec<u32>,
}

fn required_array<'a>(
    value: Option<&'a serde_json::Value>,
    name: &str,
) -> Result<&'a Vec<serde_json::Value>, String> {
    let values = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{name} must be an array"))?;
    if values.len() != CHUNK_TILE_COUNT {
        return Err(format!(
            "{name} has {} entries; expected {CHUNK_TILE_COUNT}",
            values.len()
        ));
    }
    Ok(values)
}

fn parse_u32_array(value: Option<&serde_json::Value>, name: &str) -> Result<Vec<u32>, String> {
    required_array(value, name)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| format!("{name}[{index}] must be a u32"))
        })
        .collect()
}

fn parse_u8_array(value: Option<&serde_json::Value>, name: &str) -> Result<Vec<u8>, String> {
    required_array(value, name)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| format!("{name}[{index}] must be a u8"))
        })
        .collect()
}

fn parse_optional_u16_array(
    value: Option<&serde_json::Value>,
    name: &str,
) -> Result<Vec<u16>, String> {
    let Some(value) = value else {
        return Ok(vec![0; CHUNK_TILE_COUNT]);
    };
    required_array(Some(value), name)?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_u64()
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| format!("{name}[{index}] must be a u16"))
        })
        .collect()
}

fn parse_local_coordinate(value: Option<&serde_json::Value>, name: &str) -> Result<u32, String> {
    let coordinate = value
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("{name} must be a non-negative integer"))?;
    if coordinate >= CHUNK_SIZE {
        return Err(format!(
            "{name} must be between 0 and {}; got {coordinate}",
            CHUNK_SIZE - 1
        ));
    }
    Ok(coordinate)
}

fn parse_i32(value: Option<&serde_json::Value>, name: &str) -> Result<i32, String> {
    value
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| format!("{name} must be an i32"))
}

fn parse_positive_u32(value: Option<&serde_json::Value>, name: &str) -> Result<u32, String> {
    value
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{name} must be a positive u32"))
}

/// World manager that handles loading and caching chunks
pub struct World {
    chunks: RwLock<HashMap<ChunkCoord, Arc<Chunk>>>,
    chunk_dir: String,
    /// If true, generate test chunks for missing files
    generate_missing: bool,
    /// GIDs whose map objects should not block movement
    collision_ignore_gids: HashSet<u32>,
}

impl World {
    pub fn new(chunk_dir: &str) -> Self {
        let collision_ignore_gids = Self::load_collision_ignores();

        Self {
            chunks: RwLock::new(HashMap::new()),
            chunk_dir: chunk_dir.to_string(),
            // Synthetic chunks are a development aid and must never mask missing
            // authoritative map content in an optimized server build.
            generate_missing: cfg!(debug_assertions),
            collision_ignore_gids,
        }
    }

    /// Load collision ignore list from data/collision_ignore.toml
    fn load_collision_ignores() -> HashSet<u32> {
        let path = "data/collision_ignore.toml";
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {path}: {error}"));
        let config = toml::from_str::<CollisionIgnoreConfig>(&content)
            .unwrap_or_else(|error| panic!("failed to parse {path}: {error}"));
        let gids: HashSet<u32> = config
            .ignore_object_ids
            .iter()
            .map(|id| config.objects_first_gid + id)
            .collect();
        info!(
            "Loaded {} collision-ignore GIDs (firstGid={}, {} editor IDs)",
            gids.len(),
            config.objects_first_gid,
            config.ignore_object_ids.len()
        );
        gids
    }

    /// Get or load a chunk at the given coordinates
    pub async fn get_or_load_chunk(&self, coord: ChunkCoord) -> Option<Arc<Chunk>> {
        // Check cache first
        {
            let chunks = self.chunks.read().await;
            if let Some(chunk) = chunks.get(&coord) {
                return Some(chunk.clone());
            }
        }

        let filename = format!("chunk_{}_{}.json", coord.x, coord.y);
        let file_exists = Path::new(&self.chunk_dir).join(filename).exists();

        // Try to load from file
        let chunk = self.load_chunk_from_file(coord).await;

        if let Some(chunk) = chunk {
            let chunk = Arc::new(chunk);
            let mut chunks = self.chunks.write().await;
            chunks.insert(coord, chunk.clone());
            return Some(chunk);
        }

        // An existing but invalid chunk is authoritative content corruption.
        // Never hide it behind a generated development chunk.
        if file_exists {
            return None;
        }

        // Generate test chunk if enabled
        if self.generate_missing {
            let chunk = Arc::new(Chunk::new_test(coord));
            info!("Generated test chunk at ({}, {})", coord.x, coord.y);
            let mut chunks = self.chunks.write().await;
            chunks.insert(coord, chunk.clone());
            return Some(chunk);
        }

        None
    }

    /// Load chunk from JSON file (supports both Tiled and simplified formats)
    async fn load_chunk_from_file(&self, coord: ChunkCoord) -> Option<Chunk> {
        let filename = format!("chunk_{}_{}.json", coord.x, coord.y);
        let path = Path::new(&self.chunk_dir).join(&filename);

        if !path.exists() {
            return None;
        }

        match tokio::fs::read_to_string(&path).await {
            Ok(json) => {
                // Detect format by checking for version field
                let value: serde_json::Value = match serde_json::from_str(&json) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Failed to parse JSON {:?}: {}", path, e);
                        return None;
                    }
                };

                let result = if value.get("version").is_some() {
                    // New simplified format (has version field)
                    self.parse_simplified_json(coord, &value)
                } else {
                    // Legacy Tiled format
                    self.parse_tiled_json(coord, &json)
                };

                match result {
                    Ok(mut chunk) => {
                        self.clear_ignored_collision(&mut chunk);
                        let blocked_count = chunk.collision.iter().filter(|&&b| b).count();
                        info!(
                            "Loaded chunk {:?} from {:?} - {} blocked tiles",
                            chunk.coord, path, blocked_count
                        );
                        Some(chunk)
                    }
                    Err(e) => {
                        warn!("Failed to parse chunk {:?}: {}", path, e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read chunk {:?}: {}", path, e);
                None
            }
        }
    }

    /// Parse new simplified JSON format (version 2+)
    fn parse_simplified_json(
        &self,
        coord: ChunkCoord,
        value: &serde_json::Value,
    ) -> Result<Chunk, String> {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or("missing integer version")?;
        if version != 2 {
            return Err(format!("Unsupported chunk version {version}; expected 2"));
        }
        let stored_coord = value
            .get("coord")
            .and_then(serde_json::Value::as_object)
            .ok_or("missing coord object")?;
        let stored_x = stored_coord
            .get("cx")
            .and_then(serde_json::Value::as_i64)
            .ok_or("coord.cx must be an integer")?;
        let stored_y = stored_coord
            .get("cy")
            .and_then(serde_json::Value::as_i64)
            .ok_or("coord.cy must be an integer")?;
        if stored_x != i64::from(coord.x) || stored_y != i64::from(coord.y) {
            return Err(format!(
                "Chunk coordinate mismatch: file is ({}, {}), payload is ({stored_x}, {stored_y})",
                coord.x, coord.y
            ));
        }

        let size = value
            .get("size")
            .and_then(serde_json::Value::as_u64)
            .ok_or("missing integer size")?;
        if size != u64::from(CHUNK_SIZE) {
            return Err(format!(
                "Chunk size mismatch: expected {}, got {}",
                CHUNK_SIZE, size
            ));
        }

        let mut chunk = Chunk::new(coord);

        let layers = value
            .get("layers")
            .and_then(serde_json::Value::as_object)
            .ok_or("missing layers object")?;
        chunk.layers[0].tiles = parse_u32_array(layers.get("ground"), "layers.ground")?;
        chunk.layers[1].tiles = parse_u32_array(layers.get("objects"), "layers.objects")?;
        chunk.layers[2].tiles = parse_u32_array(layers.get("overhead"), "layers.overhead")?;

        // Parse collision from base64
        let collision_b64 = value
            .get("collision")
            .and_then(serde_json::Value::as_str)
            .ok_or("collision must be a base64 string")?;
        let collision_bytes = BASE64
            .decode(collision_b64)
            .map_err(|error| format!("collision is not valid base64: {error}"))?;
        let expected_collision_bytes = CHUNK_TILE_COUNT.div_ceil(8);
        if collision_bytes.len() != expected_collision_bytes {
            return Err(format!(
                "collision has {} bytes; expected {expected_collision_bytes}",
                collision_bytes.len()
            ));
        }
        chunk.collision = Chunk::unpack_collision(&collision_bytes);

        // Parse height data (optional - missing means flat at z=0)
        if value.get("heightmap").is_some() {
            let heights = parse_u8_array(value.get("heightmap"), "heightmap")?;
            let block_types_down =
                parse_optional_u16_array(value.get("blockTypesDown"), "blockTypesDown")?;
            let block_types_right =
                parse_optional_u16_array(value.get("blockTypesRight"), "blockTypesRight")?;
            chunk.height_data = Some(crate::chunk::HeightData {
                heights,
                block_types_down,
                block_types_right,
            });
        }

        // Parse entities
        if let Some(entities_value) = value.get("entities") {
            let entities = entities_value
                .as_array()
                .ok_or("entities must be an array")?;
            for entity in entities {
                let entity_id = entity
                    .get("entityId")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or("entityId must be a non-empty string")?
                    .to_string();
                let local_x = parse_local_coordinate(entity.get("x"), "entity.x")?;
                let local_y = parse_local_coordinate(entity.get("y"), "entity.y")?;
                let world_x = coord.x * CHUNK_SIZE as i32 + local_x as i32;
                let world_y = coord.y * CHUNK_SIZE as i32 + local_y as i32;

                // Level comes from the entity prototype (TOML definitions), not from chunk JSON.
                // Chunk JSONs had "level": 1 for all entities which was overriding actual levels.
                let level: Option<i32> = None;
                let respawn = entity["respawn"].as_bool().unwrap_or(true);
                let facing = entity["facing"].as_str().map(|s| s.to_string());
                let unique_id = entity["uniqueId"].as_str().map(|s| s.to_string());

                chunk.entity_spawns.push(EntitySpawn {
                    entity_id,
                    world_x,
                    world_y,
                    level, // None = use prototype's level
                    respawn,
                    respawn_time_override: None,
                    facing,
                    unique_id,
                });
            }
        }

        // Parse map objects (new cartesian format)
        if let Some(map_objects_value) = value.get("mapObjects") {
            let map_objects = map_objects_value
                .as_array()
                .ok_or("mapObjects must be an array")?;
            for obj in map_objects {
                let gid = obj
                    .get("gid")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|gid| *gid > 0)
                    .ok_or("mapObjects.gid must be a positive u32")?;
                let local_x = parse_i32(obj.get("x"), "mapObjects.x")?;
                let local_y = parse_i32(obj.get("y"), "mapObjects.y")?;
                let world_x = coord.x * CHUNK_SIZE as i32 + local_x;
                let world_y = coord.y * CHUNK_SIZE as i32 + local_y;
                let width = parse_positive_u32(obj.get("width"), "mapObjects.width")?;
                let height = parse_positive_u32(obj.get("height"), "mapObjects.height")?;

                chunk.objects.push(MapObject {
                    gid,
                    tile_x: world_x,
                    tile_y: world_y,
                    width,
                    height,
                });
            }
        }

        // Parse walls
        if let Some(walls_value) = value.get("walls") {
            let walls_array = walls_value.as_array().ok_or("walls must be an array")?;
            for wall_value in walls_array {
                let gid = wall_value
                    .get("gid")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok())
                    .filter(|gid| *gid > 0)
                    .ok_or("walls.gid must be a positive u32")?;
                let x = parse_i32(wall_value.get("x"), "walls.x")?;
                let y = parse_i32(wall_value.get("y"), "walls.y")?;
                let edge = match wall_value.get("edge").and_then(serde_json::Value::as_str) {
                    Some("down") => WallEdge::Down,
                    Some("right") => WallEdge::Right,
                    _ => return Err("walls.edge must be 'down' or 'right'".to_string()),
                };
                chunk.walls.push(Wall {
                    gid,
                    tile_x: coord.x * CHUNK_SIZE as i32 + x,
                    tile_y: coord.y * CHUNK_SIZE as i32 + y,
                    edge,
                });
            }
        }

        // Parse portals
        let portals: Vec<Portal> = match value.get("portals") {
            Some(portals) => serde_json::from_value(portals.clone())
                .map_err(|error| format!("invalid portals: {error}"))?,
            None => Vec::new(),
        };
        for portal in &portals {
            if portal.target_map.trim().is_empty() || portal.width <= 0 || portal.height <= 0 {
                return Err(format!("invalid portal '{}'", portal.id));
            }
        }
        chunk.portals = portals;

        // Parse gathering zones
        if let Some(gathering_value) = value.get("gatheringZones") {
            let gz_array = gathering_value
                .as_array()
                .ok_or("gatheringZones must be an array")?;
            for gz in gz_array {
                let x = parse_local_coordinate(gz.get("x"), "gatheringZones.x")?;
                let y = parse_local_coordinate(gz.get("y"), "gatheringZones.y")?;
                let zone_id = gz
                    .get("zoneId")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or("gatheringZones.zoneId must be a non-empty string")?;
                chunk.gathering_zones.push(GatheringZoneMarker {
                    world_x: coord.x * CHUNK_SIZE as i32 + x as i32,
                    world_y: coord.y * CHUNK_SIZE as i32 + y as i32,
                    zone_id: zone_id.to_string(),
                });
            }
        }

        // Parse farming plots
        if let Some(plots_value) = value.get("farmingPlots") {
            let plots_array = plots_value
                .as_array()
                .ok_or("farmingPlots must be an array")?;
            for plot in plots_array {
                let id = plot
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or("farmingPlots.id must be a non-empty string")?;
                let x = parse_local_coordinate(plot.get("x"), "farmingPlots.x")?;
                let y = parse_local_coordinate(plot.get("y"), "farmingPlots.y")?;
                let patch_type = plot
                    .get("patchType")
                    .and_then(serde_json::Value::as_str)
                    .filter(|s| !s.is_empty())
                    .ok_or("farmingPlots.patchType must be a non-empty string")?;
                let read_dim = |key: &str| {
                    plot.get(key)
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok())
                };
                let width = read_dim("width").unwrap_or(1).max(1);
                let height = read_dim("height").unwrap_or(1).max(1);
                let capacity = read_dim("capacity").unwrap_or(width * height).max(1);
                chunk.farming_plots.push(crate::chunk::FarmingPlotMarker {
                    id: id.to_string(),
                    world_x: coord.x * CHUNK_SIZE as i32 + x as i32,
                    world_y: coord.y * CHUNK_SIZE as i32 + y as i32,
                    patch_type: patch_type.to_string(),
                    width,
                    height,
                    capacity,
                });
            }
        }

        Ok(chunk)
    }

    /// Parse Tiled JSON format into a Chunk
    fn parse_tiled_json(&self, coord: ChunkCoord, json: &str) -> Result<Chunk, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))?;

        let width = value["width"].as_u64().unwrap_or(CHUNK_SIZE as u64) as u32;
        let height = value["height"].as_u64().unwrap_or(CHUNK_SIZE as u64) as u32;

        if width != CHUNK_SIZE || height != CHUNK_SIZE {
            return Err(format!(
                "Chunk size mismatch: expected {}x{}, got {}x{}",
                CHUNK_SIZE, CHUNK_SIZE, width, height
            ));
        }

        let mut chunk = Chunk::new(coord);

        // Parse layers
        if let Some(layers) = value["layers"].as_array() {
            for layer_value in layers {
                let name = layer_value["name"].as_str().unwrap_or("");
                let layer_type_str = layer_value["type"].as_str().unwrap_or("");

                if layer_type_str == "tilelayer" {
                    // Get tile data
                    if let Some(data) = layer_value["data"].as_array() {
                        let tiles: Vec<u32> = data
                            .iter()
                            .map(|v| v.as_u64().unwrap_or(0) as u32)
                            .collect();

                        if tiles.len() == (CHUNK_SIZE * CHUNK_SIZE) as usize {
                            // Check if this is a collision layer
                            if name.to_lowercase().contains("collision") {
                                // Mark collision for any non-zero tile
                                for (idx, &tile_id) in tiles.iter().enumerate() {
                                    if tile_id != 0 {
                                        chunk.collision[idx] = true;
                                    }
                                }
                            } else {
                                // Regular tile layer - determine type from name
                                let layer_type = if name.to_lowercase().contains("ground") {
                                    ChunkLayerType::Ground
                                } else if name.to_lowercase().contains("overhead") {
                                    ChunkLayerType::Overhead
                                } else {
                                    ChunkLayerType::Objects
                                };

                                // Find or create layer
                                let layer_idx = chunk
                                    .layers
                                    .iter()
                                    .position(|l| l.layer_type == layer_type)
                                    .unwrap_or_else(|| {
                                        chunk.layers.push(ChunkLayer::new(layer_type));
                                        chunk.layers.len() - 1
                                    });
                                chunk.layers[layer_idx].tiles = tiles;
                            }
                        }
                    }
                } else if layer_type_str == "objectgroup" {
                    // Get layer offset (Tiled applies this to object positions)
                    let _offset_x = layer_value["offsetx"].as_f64().unwrap_or(0.0);
                    let _offset_y = layer_value["offsety"].as_f64().unwrap_or(0.0);

                    // Parse object groups for collision, spawn points, and map objects
                    if let Some(objects) = layer_value["objects"].as_array() {
                        for obj in objects {
                            let obj_type = obj["type"].as_str().unwrap_or("");
                            let pixel_x = obj["x"].as_f64().unwrap_or(0.0);
                            let pixel_y = obj["y"].as_f64().unwrap_or(0.0);
                            let obj_width = obj["width"].as_f64().unwrap_or(0.0);
                            let obj_height = obj["height"].as_f64().unwrap_or(0.0);

                            // Check if this object has a gid (it's a tileset object like tree/rock)
                            if let Some(gid) = obj["gid"].as_u64() {
                                // Tiled isometric: pixel coords use tileHeight (32) for both
                                let tile_x = (pixel_x / 32.0).floor() as i32;
                                let tile_y = (pixel_y / 32.0).floor() as i32;

                                // Convert to world coordinates (add chunk offset)
                                let world_tile_x = coord.x * CHUNK_SIZE as i32 + tile_x;
                                let world_tile_y = coord.y * CHUNK_SIZE as i32 + tile_y;

                                info!(
                                    "SERVER Object gid {} | pixel ({:.1}, {:.1}) | local ({}, {}) | chunk ({}, {}) | WORLD ({}, {})",
                                    gid,
                                    pixel_x,
                                    pixel_y,
                                    tile_x,
                                    tile_y,
                                    coord.x,
                                    coord.y,
                                    world_tile_x,
                                    world_tile_y
                                );

                                chunk.objects.push(MapObject {
                                    gid: gid as u32,
                                    tile_x: world_tile_x,
                                    tile_y: world_tile_y,
                                    width: obj_width as u32,
                                    height: obj_height as u32,
                                });
                            } else {
                                // Legacy handling for non-gid objects (collision, spawn points)
                                let x = (pixel_x / 64.0) as u32;
                                let y = (pixel_y / 32.0) as u32;
                                let collision_width = (obj_width / 64.0).ceil() as u32;
                                let collision_height = (obj_height / 32.0).ceil() as u32;

                                if obj_type == "collision"
                                    || name.to_lowercase().contains("collision")
                                {
                                    // Mark collision area
                                    for dy in 0..collision_height.max(1) {
                                        for dx in 0..collision_width.max(1) {
                                            chunk.set_collision(x + dx, y + dy, true);
                                        }
                                    }
                                } else if obj_type == "entity_spawn" || obj_type == "npc_spawn" {
                                    // Parse entity spawn from Tiled object properties
                                    if let Some(spawn) = self.parse_entity_spawn(obj, coord) {
                                        info!(
                                            "Found entity spawn: {} at ({}, {})",
                                            spawn.entity_id, spawn.world_x, spawn.world_y
                                        );
                                        chunk.entity_spawns.push(spawn);
                                    }
                                } else if obj_type == "player_spawn"
                                    || obj_type == "spawn"
                                    || name.to_lowercase().contains("spawn")
                                {
                                    chunk.player_spawns.push((x, y));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(chunk)
    }

    /// Parse an entity spawn object from Tiled JSON
    fn parse_entity_spawn(
        &self,
        obj: &serde_json::Value,
        chunk_coord: ChunkCoord,
    ) -> Option<EntitySpawn> {
        // Get pixel position and convert to grid
        let pixel_x = obj["x"].as_f64()?;
        let pixel_y = obj["y"].as_f64()?;
        let local_x = (pixel_x / 64.0) as u32;
        let local_y = (pixel_y / 32.0) as u32;

        // Convert to world coordinates
        let world_x = chunk_coord.x * CHUNK_SIZE as i32 + local_x as i32;
        let world_y = chunk_coord.y * CHUNK_SIZE as i32 + local_y as i32;

        // Parse properties array
        let properties = obj["properties"].as_array();

        // Extract entity_id (required)
        let entity_id = self.get_property_string(properties, "entity_id")?;

        // Extract optional properties
        let level = self.get_property_int(properties, "level"); // None = use prototype's level
        let respawn = self
            .get_property_bool(properties, "respawn")
            .unwrap_or(true);
        let respawn_time_override = self
            .get_property_int(properties, "respawn_time_override")
            .map(|v| v as u64);
        let facing = self.get_property_string(properties, "facing");
        let unique_id = self.get_property_string(properties, "unique_id");

        Some(EntitySpawn {
            entity_id,
            world_x,
            world_y,
            level,
            respawn,
            respawn_time_override,
            facing,
            unique_id,
        })
    }

    /// Get a string property from Tiled properties array
    fn get_property_string(
        &self,
        props: Option<&Vec<serde_json::Value>>,
        name: &str,
    ) -> Option<String> {
        props?
            .iter()
            .find(|p| p["name"].as_str() == Some(name))
            .and_then(|p| p["value"].as_str().map(|s| s.to_string()))
    }

    /// Get an int property from Tiled properties array
    fn get_property_int(&self, props: Option<&Vec<serde_json::Value>>, name: &str) -> Option<i32> {
        props?
            .iter()
            .find(|p| p["name"].as_str() == Some(name))
            .and_then(|p| p["value"].as_i64().map(|i| i as i32))
    }

    /// Get a bool property from Tiled properties array
    fn get_property_bool(
        &self,
        props: Option<&Vec<serde_json::Value>>,
        name: &str,
    ) -> Option<bool> {
        props?
            .iter()
            .find(|p| p["name"].as_str() == Some(name))
            .and_then(|p| p["value"].as_bool())
    }

    /// Clear collision bits for map objects whose GID is in the ignore list
    fn clear_ignored_collision(&self, chunk: &mut Chunk) {
        if self.collision_ignore_gids.is_empty() {
            return;
        }

        let tiles_to_clear: Vec<(u32, u32)> = chunk
            .objects
            .iter()
            .filter(|obj| self.collision_ignore_gids.contains(&obj.gid))
            .map(|obj| world_to_local(obj.tile_x, obj.tile_y))
            .collect();

        for (lx, ly) in tiles_to_clear {
            chunk.set_collision(lx, ly, false);
        }
    }

    /// Get a snapshot of all currently loaded chunks for synchronous access
    pub async fn chunks_snapshot(&self) -> HashMap<ChunkCoord, Arc<Chunk>> {
        self.chunks.read().await.clone()
    }

    /// Get a read guard to the loaded chunks (avoids cloning the entire map)
    pub async fn chunks_read(
        &self,
    ) -> tokio::sync::RwLockReadGuard<'_, HashMap<ChunkCoord, Arc<Chunk>>> {
        self.chunks.read().await
    }

    /// Check if a world position is walkable (loads chunk from disk if needed)
    pub async fn is_tile_walkable(&self, world_x: i32, world_y: i32) -> bool {
        let coord = ChunkCoord::from_world(world_x, world_y);

        if let Some(chunk) = self.get_or_load_chunk(coord).await {
            let (local_x, local_y) = world_to_local(world_x, world_y);
            chunk.is_walkable_local(local_x, local_y)
        } else {
            false // Non-existent chunks are impassable
        }
    }

    /// Check if a world position is walkable using only already-loaded chunks.
    /// Returns false for tiles in unloaded chunks (no disk I/O).
    pub async fn is_tile_walkable_loaded(&self, world_x: i32, world_y: i32) -> bool {
        let coord = ChunkCoord::from_world(world_x, world_y);
        let chunks = self.chunks.read().await;
        if let Some(chunk) = chunks.get(&coord) {
            let (local_x, local_y) = world_to_local(world_x, world_y);
            chunk.is_walkable_local(local_x, local_y)
        } else {
            false
        }
    }

    /// Get the terrain height at a world position from loaded chunks.
    /// Returns 0 for tiles in unloaded chunks or chunks without height data.
    pub fn get_height_at_sync(
        &self,
        world_x: i32,
        world_y: i32,
        chunks: &std::collections::HashMap<ChunkCoord, std::sync::Arc<crate::chunk::Chunk>>,
    ) -> i32 {
        let coord = ChunkCoord::from_world(world_x, world_y);
        if let Some(chunk) = chunks.get(&coord) {
            let (local_x, local_y) = world_to_local(world_x, world_y);
            chunk.get_height(local_x, local_y) as i32
        } else {
            0
        }
    }

    /// Check if there's a clear line of sight between two points (Bresenham's line)
    /// Returns true if no solid tiles block the path
    pub async fn has_line_of_sight(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut x = x0;
        let mut y = y0;

        loop {
            // Skip start and end positions — only check intermediate tiles for obstacles.
            // The target tile may be a collision tile (e.g., NPC standing on rocks)
            // but we still want to allow attacks to reach it.
            if (x != x0 || y != y0) && (x != x1 || y != y1) && !self.is_tile_walkable(x, y).await {
                return false;
            }

            if x == x1 && y == y1 {
                return true;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 {
                    return true;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    return true;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Get chunks in a radius around a center coordinate
    pub fn get_chunks_in_radius(&self, center: ChunkCoord, radius: i32) -> Vec<ChunkCoord> {
        center.in_radius(radius)
    }

    /// Preload chunks around a position
    pub async fn preload_chunks(&self, center: ChunkCoord, radius: i32) {
        for coord in center.in_radius(radius) {
            let _ = self.get_or_load_chunk(coord).await;
        }
    }

    /// Get a safe spawn position in the world (prefers chunk 0,0)
    pub async fn get_spawn_position(&self) -> (i32, i32) {
        (-30, 19)
    }

    /// Check if a chunk file exists on disk (ignores generate_missing flag).
    /// Used for validating saved player positions on login.
    pub fn chunk_file_exists(&self, coord: ChunkCoord) -> bool {
        let filename = format!("chunk_{}_{}.json", coord.x, coord.y);
        let path = Path::new(&self.chunk_dir).join(&filename);
        path.exists()
    }

    /// Check if a chunk exists (either loaded or loadable)
    pub async fn chunk_exists(&self, coord: ChunkCoord) -> bool {
        // Check cache
        {
            let chunks = self.chunks.read().await;
            if chunks.contains_key(&coord) {
                return true;
            }
        }

        // Check file
        let filename = format!("chunk_{}_{}.json", coord.x, coord.y);
        let path = Path::new(&self.chunk_dir).join(&filename);

        if path.exists() {
            return true;
        }

        // If generating missing chunks, always return true
        self.generate_missing
    }

    /// Get chunk data for sending to client
    pub async fn get_chunk_data(&self, coord: ChunkCoord) -> Option<Arc<Chunk>> {
        self.get_or_load_chunk(coord).await
    }

    /// Get number of loaded chunks
    pub async fn loaded_chunk_count(&self) -> usize {
        self.chunks.read().await.len()
    }

    /// Unload chunks outside of active area (for memory management)
    pub async fn unload_distant_chunks(&self, active_coords: &[ChunkCoord], keep_radius: i32) {
        let mut chunks = self.chunks.write().await;
        chunks.retain(|coord, _| {
            active_coords.iter().any(|active| {
                (coord.x - active.x).abs() <= keep_radius
                    && (coord.y - active.y).abs() <= keep_radius
            })
        });
    }

    /// Discover all chunk files in the chunk directory and return their coordinates
    pub fn discover_chunk_coords(&self) -> Vec<ChunkCoord> {
        let mut coords = Vec::new();
        let path = Path::new(&self.chunk_dir);

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let filename = entry.file_name();
                let filename_str = filename.to_string_lossy();

                // Parse chunk_X_Y.json format
                if let Some(name) = filename_str.strip_prefix("chunk_")
                    && let Some(name) = name.strip_suffix(".json")
                {
                    let parts: Vec<&str> = name.split('_').collect();
                    if parts.len() == 2
                        && let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                    {
                        coords.push(ChunkCoord::new(x, y));
                    }
                }
            }
        }

        coords
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_world_generates_missing_chunks() {
        let world = World::new("nonexistent_dir");
        let chunk = world.get_or_load_chunk(ChunkCoord::new(0, 0)).await;
        assert!(chunk.is_some());
    }

    #[tokio::test]
    async fn test_world_walkability() {
        let world = World::new("nonexistent_dir");

        // Load chunk to ensure it exists
        let _ = world.get_or_load_chunk(ChunkCoord::new(0, 0)).await;

        // Center should be walkable
        assert!(world.is_tile_walkable(16, 16).await);
    }

    #[tokio::test]
    async fn test_preload_chunks() {
        let world = World::new("nonexistent_dir");
        world.preload_chunks(ChunkCoord::new(0, 0), 1).await;

        // Should have 9 chunks loaded (3x3)
        assert_eq!(world.loaded_chunk_count().await, 9);
    }

    #[test]
    fn parse_simplified_json_reads_farming_plots() {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let world = World::new("nonexistent_dir");
        let coord = ChunkCoord::new(-1, -1);
        let n = (CHUNK_SIZE * CHUNK_SIZE) as usize;
        let layer: Vec<u32> = vec![0; n];
        let collision = BASE64.encode(vec![0u8; n.div_ceil(8)]);
        let value = serde_json::json!({
            "version": 2,
            "coord": { "cx": -1, "cy": -1 },
            "size": CHUNK_SIZE,
            "layers": { "ground": layer, "objects": layer, "overhead": layer },
            "collision": collision,
            "farmingPlots": [
                { "id": "fp_bed", "x": 25, "y": 2, "width": 2, "height": 2, "patchType": "allotment", "capacity": 4 },
                { "id": "fp_herb", "x": 30, "y": 4, "patchType": "herb" }
            ]
        });

        let chunk = world.parse_simplified_json(coord, &value).unwrap();
        assert_eq!(chunk.farming_plots.len(), 2);

        let bed = &chunk.farming_plots[0];
        assert_eq!(bed.id, "fp_bed");
        assert_eq!(bed.patch_type, "allotment");
        // local (25, 2) in chunk (-1, -1) -> world (-7, -30)
        assert_eq!((bed.world_x, bed.world_y), (-7, -30));
        assert_eq!((bed.width, bed.height, bed.capacity), (2, 2, 4));

        // herb omits dims -> defaults to 1x1 capacity 1
        let herb = &chunk.farming_plots[1];
        assert_eq!((herb.width, herb.height, herb.capacity), (1, 1, 1));
    }
}
