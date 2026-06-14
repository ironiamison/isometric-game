use std::collections::HashMap;

pub const CHUNK_SIZE: u32 = 32;
const WORLD_VIEW_RADIUS: i32 = 2;
const MINIMAP_VISIBLE_RADIUS: i32 = 2;
const MINIMAP_PRELOAD_RING: i32 = 1;
const MAX_CHUNK_REQUESTS_PER_UPDATE: usize = 2;

/// Chunk coordinates in the world grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Convert world position to chunk coordinate
    pub fn from_world(world_x: i32, world_y: i32) -> Self {
        Self {
            x: world_x.div_euclid(CHUNK_SIZE as i32),
            y: world_y.div_euclid(CHUNK_SIZE as i32),
        }
    }

    /// Convert float world position to chunk coordinate
    pub fn from_world_f32(world_x: f32, world_y: f32) -> Self {
        Self::from_world(world_x.floor() as i32, world_y.floor() as i32)
    }
}

/// Convert world position to local chunk position
pub fn world_to_local(world_x: i32, world_y: i32) -> (u32, u32) {
    (
        world_x.rem_euclid(CHUNK_SIZE as i32) as u32,
        world_y.rem_euclid(CHUNK_SIZE as i32) as u32,
    )
}

/// Map object placed from Tiled's object layer (trees, rocks, decorations)
#[derive(Debug, Clone)]
pub struct MapObject {
    /// Global tile ID from objects.tsx tileset
    pub gid: u32,
    /// World tile X coordinate
    pub tile_x: i32,
    /// World tile Y coordinate
    pub tile_y: i32,
    /// Sprite width in pixels
    pub width: u32,
    /// Sprite height in pixels
    pub height: u32,
}

/// Wall edge direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallEdge {
    Down,
    Right,
}

/// Wall placed on a tile edge
#[derive(Debug, Clone)]
pub struct Wall {
    /// Global tile ID for wall sprite
    pub gid: u32,
    /// World tile X coordinate
    pub tile_x: i32,
    /// World tile Y coordinate
    pub tile_y: i32,
    /// Which edge of the tile this wall is on
    pub edge: WallEdge,
}

/// A portal that teleports players to another map
#[derive(Debug, Clone)]
pub struct Portal {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub target_map: String,
    pub target_spawn: String,
}

/// Layer types matching server
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkLayerType {
    Ground = 0,
    Objects = 1,
    Overhead = 2,
}

impl ChunkLayerType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => ChunkLayerType::Ground,
            1 => ChunkLayerType::Objects,
            2 => ChunkLayerType::Overhead,
            _ => ChunkLayerType::Ground,
        }
    }
}

/// A layer of tiles within a chunk
#[derive(Debug, Clone)]
pub struct ChunkLayer {
    pub layer_type: ChunkLayerType,
    pub tiles: Vec<u32>,
}

impl ChunkLayer {
    pub fn new(layer_type: ChunkLayerType) -> Self {
        Self {
            layer_type,
            tiles: vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize],
        }
    }

    pub fn get_tile(&self, local_x: u32, local_y: u32) -> u32 {
        if local_x >= CHUNK_SIZE || local_y >= CHUNK_SIZE {
            return 0;
        }
        let idx = (local_y * CHUNK_SIZE + local_x) as usize;
        self.tiles.get(idx).copied().unwrap_or(0)
    }
}

/// Client-side chunk data
#[derive(Debug, Clone)]
pub struct Chunk {
    pub coord: ChunkCoord,
    pub layers: Vec<ChunkLayer>,
    pub collision: Vec<bool>,
    /// Map objects (trees, rocks, decorations) from object layer
    pub objects: Vec<MapObject>,
    /// Walls placed on tile edges
    pub walls: Vec<Wall>,
    /// Portals that teleport players to other maps
    pub portals: Vec<Portal>,
    /// Optional heightmap data (CHUNK_SIZE^2 u8 values, None = flat z=0)
    pub heights: Option<Vec<u8>>,
    /// Optional block type data for down (+Y) side face (CHUNK_SIZE^2 u16 values)
    pub block_types_down: Option<Vec<u16>>,
    /// Optional block type data for right (+X) side face (CHUNK_SIZE^2 u16 values)
    pub block_types_right: Option<Vec<u16>>,
}

impl Chunk {
    pub fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            layers: vec![
                ChunkLayer::new(ChunkLayerType::Ground),
                ChunkLayer::new(ChunkLayerType::Objects),
                ChunkLayer::new(ChunkLayerType::Overhead),
            ],
            collision: vec![false; (CHUNK_SIZE * CHUNK_SIZE) as usize],
            objects: Vec::new(),
            walls: Vec::new(),
            portals: Vec::new(),
            heights: None,
            block_types_down: None,
            block_types_right: None,
        }
    }

    /// Get tile from a specific layer
    pub fn get_tile(&self, layer_type: ChunkLayerType, local_x: u32, local_y: u32) -> u32 {
        for layer in &self.layers {
            if layer.layer_type == layer_type {
                return layer.get_tile(local_x, local_y);
            }
        }
        0
    }

    /// Check if a local position is walkable
    pub fn is_walkable_local(&self, local_x: u32, local_y: u32) -> bool {
        if local_x >= CHUNK_SIZE || local_y >= CHUNK_SIZE {
            return false;
        }
        let idx = (local_y * CHUNK_SIZE + local_x) as usize;
        !self.collision.get(idx).copied().unwrap_or(true)
    }

    /// Get height at a local tile position (returns 0 if no heightmap)
    pub fn get_height(&self, local_x: u32, local_y: u32) -> u8 {
        if let Some(ref heights) = self.heights {
            let index = (local_y * CHUNK_SIZE + local_x) as usize;
            heights.get(index).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Get block type for down (+Y) face at a local tile position (returns 0 if no data)
    pub fn get_block_type_down(&self, local_x: u32, local_y: u32) -> u16 {
        if let Some(ref bt) = self.block_types_down {
            let index = (local_y * CHUNK_SIZE + local_x) as usize;
            bt.get(index).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Get block type for right (+X) face at a local tile position (returns 0 if no data)
    pub fn get_block_type_right(&self, local_x: u32, local_y: u32) -> u16 {
        if let Some(ref bt) = self.block_types_right {
            let index = (local_y * CHUNK_SIZE + local_x) as usize;
            bt.get(index).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Unpack collision data from server bytes
    pub fn unpack_collision(packed: &[u8]) -> Vec<bool> {
        let mut collision = vec![false; (CHUNK_SIZE * CHUNK_SIZE) as usize];
        for (i, blocked) in collision.iter_mut().enumerate() {
            if i / 8 < packed.len() {
                *blocked = (packed[i / 8] >> (i % 8)) & 1 == 1;
            }
        }
        collision
    }

    /// Unpack collision data for interiors with custom size
    pub fn unpack_collision_sized(packed: &[u8], tile_count: usize) -> Vec<bool> {
        let mut collision = vec![false; tile_count];
        for (i, blocked) in collision.iter_mut().enumerate() {
            if i / 8 < packed.len() {
                *blocked = (packed[i / 8] >> (i % 8)) & 1 == 1;
            }
        }
        collision
    }
}

/// Chunk manager handles loading and caching chunks
pub struct ChunkManager {
    chunks: HashMap<ChunkCoord, Chunk>,
    /// Chunks that we've requested but haven't received yet
    pending_requests: HashMap<ChunkCoord, f64>,
    /// Current player chunk for tracking movement
    current_chunk: ChunkCoord,
    /// View radius in chunks
    view_radius: i32,
    /// Interior mode - if Some, we're in an interior with (width, height)
    interior_size: Option<(u32, u32)>,
    /// Cached overworld chunks while inside an interior
    overworld_cache: Option<OverworldCache>,
    /// Bumped whenever the loaded chunk set changes (load/unload/interior swap).
    /// Lets renderers cache derived data (e.g. the minimap raster) and invalidate
    /// only when the underlying tiles actually change.
    revision: u64,
}

struct OverworldCache {
    chunks: HashMap<ChunkCoord, Chunk>,
    current_chunk: ChunkCoord,
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            pending_requests: HashMap::new(),
            current_chunk: ChunkCoord::new(0, 0),
            // Keep one extra chunk ring loaded beyond the visible minimap edge.
            view_radius: WORLD_VIEW_RADIUS.max(MINIMAP_VISIBLE_RADIUS + MINIMAP_PRELOAD_RING),
            interior_size: None,
            overworld_cache: None,
            revision: 0,
        }
    }

    /// Monotonic counter that changes whenever the loaded chunk set changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Check if we're in an interior and get its size
    pub fn get_interior_size(&self) -> Option<(u32, u32)> {
        self.interior_size
    }

    /// Check if we're in an interior
    pub fn is_interior(&self) -> bool {
        self.interior_size.is_some()
    }

    /// Update player position and return list of chunks to request
    pub fn update_player_position(&mut self, world_x: f32, world_y: f32) -> Vec<ChunkCoord> {
        // Don't request world chunks when in interior mode
        if self.interior_size.is_some() {
            return Vec::new();
        }

        let new_chunk = ChunkCoord::from_world_f32(world_x, world_y);
        self.current_chunk = new_chunk;

        // Check which chunks need to be loaded
        let mut candidates = Vec::new();
        let current_time = macroquad::time::get_time();

        for dy in -self.view_radius..=self.view_radius {
            for dx in -self.view_radius..=self.view_radius {
                let coord = ChunkCoord::new(new_chunk.x + dx, new_chunk.y + dy);

                // Skip if already loaded
                if self.chunks.contains_key(&coord) {
                    continue;
                }

                // Skip if recently requested (wait 2 seconds before re-requesting)
                if let Some(&request_time) = self.pending_requests.get(&coord) {
                    if current_time - request_time < 2.0 {
                        continue;
                    }
                }

                candidates.push(coord);
            }
        }

        // Prioritize nearby chunks first and throttle per-frame requests
        // to avoid large burst loads that cause frame hitches.
        candidates.sort_by_key(|coord| {
            (coord.x - new_chunk.x)
                .abs()
                .max((coord.y - new_chunk.y).abs())
        });

        let mut to_request = Vec::new();
        for coord in candidates.into_iter().take(MAX_CHUNK_REQUESTS_PER_UPDATE) {
            self.pending_requests.insert(coord, current_time);
            to_request.push(coord);
        }

        to_request
    }

    /// Load chunk data received from server
    pub fn load_chunk(
        &mut self,
        chunk_x: i32,
        chunk_y: i32,
        layers: Vec<(u8, Vec<u32>)>,
        collision: &[u8],
        objects: Vec<MapObject>,
        walls: Vec<Wall>,
        portals: Vec<Portal>,
        heights: Option<Vec<u8>>,
        block_types_down: Option<Vec<u16>>,
        block_types_right: Option<Vec<u16>>,
    ) {
        // Don't load world chunks when in interior mode
        if self.interior_size.is_some() {
            log::debug!(
                "Ignoring world chunk ({}, {}) while in interior mode",
                chunk_x,
                chunk_y
            );
            return;
        }

        let coord = ChunkCoord::new(chunk_x, chunk_y);
        let mut chunk = Chunk::new(coord);

        // Load layers
        for (layer_type, tiles) in layers {
            let lt = ChunkLayerType::from_u8(layer_type);
            for layer in &mut chunk.layers {
                if layer.layer_type == lt {
                    layer.tiles = tiles.clone();
                    break;
                }
            }
        }

        // Load collision
        chunk.collision = Chunk::unpack_collision(collision);

        // Load map objects
        chunk.objects = objects;

        // Load walls
        chunk.walls = walls;

        // Load portals
        chunk.portals = portals;

        // Load optional height data
        chunk.heights = heights;
        chunk.block_types_down = block_types_down;
        chunk.block_types_right = block_types_right;

        // Remove from pending
        self.pending_requests.remove(&coord);

        let _object_count = chunk.objects.len();
        let _portal_count = chunk.portals.len();

        // Store chunk
        self.chunks.insert(coord, chunk);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Pick the tile under a screen position, accounting for elevation.
    /// Probes from highest Z down to find the topmost elevated tile the cursor is over.
    /// Returns (tile_x, tile_y, tile_z).
    pub fn pick_tile_at_screen(
        &self,
        screen_x: f32,
        screen_y: f32,
        camera: &crate::game::Camera,
    ) -> (i32, i32, i32) {
        use crate::render::isometric::{screen_to_world, world_to_screen_z_exact, TILE_HEIGHT};

        // Try elevated tiles first (highest Z down to 1)
        const MAX_PROBE_Z: i32 = 15;
        for probe_z in (1..=MAX_PROBE_Z).rev() {
            let z_offset = probe_z as f32 * (TILE_HEIGHT / 2.0) * camera.zoom;
            let (wx, wy) = screen_to_world(screen_x, screen_y + z_offset, camera);
            let tx = wx.round() as i32;
            let ty = wy.round() as i32;
            let terrain_h = self.get_height(tx, ty) as i32;
            if terrain_h == probe_z {
                // Verify cursor is on the tile's diamond surface, not on
                // the block side face below it. The diamond extends
                // TILE_HEIGHT/4 below its center.
                let (_, center_sy) = world_to_screen_z_exact(
                    tx as f32 + 0.5,
                    ty as f32 + 0.5,
                    probe_z as f32,
                    camera,
                );
                let half_diamond = (TILE_HEIGHT / 4.0) * camera.zoom;
                if screen_y > center_sy + half_diamond {
                    continue; // Cursor is below the surface (on block side)
                }
                return (tx, ty, probe_z);
            }
        }

        // Fall back to ground level
        let (wx, wy) = screen_to_world(screen_x, screen_y, camera);
        let tx = wx.round() as i32;
        let ty = wy.round() as i32;
        let z = self.get_height(tx, ty) as i32;
        (tx, ty, z)
    }

    /// Get terrain height at a world position (returns 0 if no heightmap or unloaded)
    pub fn get_height(&self, world_x: i32, world_y: i32) -> u8 {
        if self.interior_size.is_some() {
            return 0; // Interiors don't use heightmaps
        }
        let coord = ChunkCoord::from_world(world_x, world_y);
        if let Some(chunk) = self.chunks.get(&coord) {
            let (local_x, local_y) = world_to_local(world_x, world_y);
            chunk.get_height(local_x, local_y)
        } else {
            0
        }
    }

    /// Check if a world position is walkable
    pub fn is_walkable(&self, world_x: f32, world_y: f32) -> bool {
        let x = world_x.floor() as i32;
        let y = world_y.floor() as i32;

        // For interiors, use interior dimensions for collision checking
        if let Some((width, height)) = self.interior_size {
            // In interior mode, chunk is at (0,0) and coords are direct
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                return false; // Out of bounds
            }
            let coord = ChunkCoord::new(0, 0);
            if let Some(chunk) = self.chunks.get(&coord) {
                let idx = (y as u32 * width + x as u32) as usize;
                let blocked = chunk.collision.get(idx).copied().unwrap_or(true);
                return !blocked;
            }
            return false;
        }

        // Standard chunk-based collision - if we get here, we're NOT in interior mode
        let coord = ChunkCoord::from_world(x, y);
        if let Some(chunk) = self.chunks.get(&coord) {
            let (local_x, local_y) = world_to_local(x, y);
            chunk.is_walkable_local(local_x, local_y)
        } else {
            false // Unloaded chunks are impassable
        }
    }

    /// Get tile at world position
    pub fn get_tile(&self, layer_type: ChunkLayerType, world_x: i32, world_y: i32) -> u32 {
        let coord = ChunkCoord::from_world(world_x, world_y);

        if let Some(chunk) = self.chunks.get(&coord) {
            let (local_x, local_y) = world_to_local(world_x, world_y);
            chunk.get_tile(layer_type, local_x, local_y)
        } else {
            0
        }
    }

    /// Get all loaded chunks for rendering
    pub fn chunks(&self) -> &HashMap<ChunkCoord, Chunk> {
        &self.chunks
    }

    /// Check if a chunk is loaded
    pub fn is_chunk_loaded(&self, coord: ChunkCoord) -> bool {
        self.chunks.contains_key(&coord)
    }

    /// Get number of loaded chunks
    pub fn loaded_count(&self) -> usize {
        self.chunks.len()
    }

    /// Unload chunks far from current position
    pub fn unload_distant_chunks(&mut self) {
        let keep_radius = self.view_radius + 1;
        let current = self.current_chunk;

        let before = self.chunks.len();
        self.chunks.retain(|coord, _| {
            (coord.x - current.x).abs() <= keep_radius && (coord.y - current.y).abs() <= keep_radius
        });
        if self.chunks.len() != before {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Find a portal at the given world position
    pub fn get_portal_at(&self, x: f32, y: f32) -> Option<&Portal> {
        let tile_x = x.floor() as i32;
        let tile_y = y.floor() as i32;

        // In interior mode, all data is in chunk (0,0) and portal coords are absolute
        if self.interior_size.is_some() {
            let chunk = self.chunks.get(&ChunkCoord { x: 0, y: 0 })?;
            return chunk.portals.iter().find(|p| {
                tile_x >= p.x && tile_x < p.x + p.width && tile_y >= p.y && tile_y < p.y + p.height
            });
        }

        let coord = ChunkCoord::from_world_f32(x, y);
        let chunk = self.chunks.get(&coord)?;

        // Portal x/y are LOCAL coords (0-31), need to convert to WORLD coords
        let chunk_base_x = coord.x * CHUNK_SIZE as i32;
        let chunk_base_y = coord.y * CHUNK_SIZE as i32;

        chunk.portals.iter().find(|p| {
            let world_px = chunk_base_x + p.x;
            let world_py = chunk_base_y + p.y;
            tile_x >= world_px
                && tile_x < world_px + p.width
                && tile_y >= world_py
                && tile_y < world_py + p.height
        })
    }

    /// Find a map object at the given world tile position
    /// This checks exact matches first, then searches for tall objects (like trees)
    /// that visually cover the target tile based on their height.
    pub fn get_object_at(&self, tile_x: i32, tile_y: i32) -> Option<&MapObject> {
        let coord = ChunkCoord::from_world(tile_x, tile_y);
        let chunk = self.chunks.get(&coord)?;

        // First try exact match
        if let Some(obj) = chunk
            .objects
            .iter()
            .find(|obj| obj.tile_x == tile_x && obj.tile_y == tile_y)
        {
            return Some(obj);
        }

        // For tall objects (trees, etc.), check if target tile is covered by the object's height
        // Objects are anchored at their base (tile_x, tile_y) and extend upward (smaller Y values)
        // A tree at (10, 15) with height 96px (3 tiles) covers Y: 13, 14, 15
        const TILE_HEIGHT_PX: u32 = 32;

        chunk.objects.iter().find(|obj| {
            if obj.tile_x != tile_x {
                return false;
            }
            // Calculate how many tiles upward this object extends
            let tiles_high = obj.height.div_ceil(TILE_HEIGHT_PX) as i32;
            // Object covers from (tile_y - tiles_high + 1) to tile_y
            let min_y = obj.tile_y - tiles_high + 1;
            tile_y >= min_y && tile_y <= obj.tile_y
        })
    }

    /// Get object at exact tile position only (no tall-object extension)
    /// Use this for UI elements like name tags that should only appear on the object's base tile
    pub fn get_object_at_exact(&self, tile_x: i32, tile_y: i32) -> Option<&MapObject> {
        let coord = ChunkCoord::from_world(tile_x, tile_y);
        let chunk = self.chunks.get(&coord)?;
        chunk
            .objects
            .iter()
            .find(|obj| obj.tile_x == tile_x && obj.tile_y == tile_y)
    }

    /// Load an interior as a single chunk at (0,0)
    pub fn load_interior(
        &mut self,
        width: u32,
        height: u32,
        layers: Vec<(u8, Vec<u32>)>,
        collision: &[u8],
        portals: Vec<Portal>,
        objects: Vec<MapObject>,
        walls: Vec<Wall>,
        heightmap: Option<Vec<u8>>,
        block_types_down: Option<Vec<u16>>,
        block_types_right: Option<Vec<u16>>,
    ) {
        // Cache overworld chunks when entering an interior
        if self.interior_size.is_none() {
            self.overworld_cache = Some(OverworldCache {
                chunks: std::mem::take(&mut self.chunks),
                current_chunk: self.current_chunk,
            });
        } else {
            self.chunks.clear();
        }
        self.pending_requests.clear();

        // Set interior mode with dimensions
        self.interior_size = Some((width, height));

        // Create interior chunk at (0,0)
        let coord = ChunkCoord { x: 0, y: 0 };

        let chunk_layers: Vec<ChunkLayer> = layers
            .into_iter()
            .map(|(layer_type, tiles)| ChunkLayer {
                layer_type: ChunkLayerType::from_u8(layer_type),
                tiles,
            })
            .collect();

        let collision_data = Chunk::unpack_collision_sized(collision, (width * height) as usize);

        let chunk = Chunk {
            coord,
            layers: chunk_layers,
            collision: collision_data,
            objects,
            walls,
            portals,
            heights: heightmap,
            block_types_down,
            block_types_right,
        };

        self.chunks.insert(coord, chunk);
        self.current_chunk = coord;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Clear interior and prepare for overworld
    pub fn clear_interior(&mut self) {
        self.interior_size = None;
        self.pending_requests.clear();
        if let Some(cache) = self.overworld_cache.take() {
            self.chunks = cache.chunks;
            self.current_chunk = cache.current_chunk;
        } else {
            self.chunks.clear();
        }
        self.revision = self.revision.wrapping_add(1);
    }
}
