pub const WORLD_MAP_POI_KIND_TREE: u8 = 0;
pub const WORLD_MAP_POI_KIND_TELEPORT: u8 = 1;
pub const WORLD_MAP_POI_KIND_QUEST: u8 = 2;
pub const WORLD_MAP_POI_KIND_SERVICE: u8 = 3;
pub const WORLD_MAP_POI_KIND_CHEST: u8 = 4;

#[derive(Clone, Copy, Debug)]
pub struct WorldMapBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl WorldMapBounds {
    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }
}

#[derive(Clone, Debug)]
pub struct WorldMapChunkSample {
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub low_tiles: Vec<u32>,
    pub high_tiles: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct WorldMapPoi {
    pub x: f32,
    pub y: f32,
    pub label: String,
    pub icon_index: u8,
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct WorldMapSnapshot {
    pub bounds: WorldMapBounds,
    pub low_sample_dim: usize,
    pub high_sample_dim: usize,
    pub chunks: Vec<WorldMapChunkSample>,
    pub pois: Vec<WorldMapPoi>,
}
