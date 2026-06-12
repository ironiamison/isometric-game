use crate::chunk::{CHUNK_SIZE, ChunkCoord, ChunkLayerType};
use crate::game::GameRoom;
use crate::npc::Npc;
use crate::protocol::{ServerMessage, WorldMapChunkSampleData, WorldMapPoiData};
use crate::world::World;
use std::collections::HashMap;
use std::sync::Arc;

const WORLD_MAP_LOW_SAMPLE_DIM: usize = 4;
const WORLD_MAP_HIGH_SAMPLE_DIM: usize = 8;

const WORLD_MAP_POI_TREE: u8 = 0;
const WORLD_MAP_POI_TELEPORT: u8 = 1;
const WORLD_MAP_POI_QUEST: u8 = 2;
const WORLD_MAP_POI_SERVICE: u8 = 3;
const WORLD_MAP_POI_CHEST: u8 = 4;

fn format_map_display_name(target_map: &str) -> String {
    let raw = target_map.trim();
    if raw.is_empty() {
        return "Unknown".to_string();
    }

    let scoped = raw.rsplit(':').next().unwrap_or(raw);
    let id = scoped.rsplit('/').next().unwrap_or(scoped).trim();
    if id.is_empty() {
        return "Unknown".to_string();
    }

    if id.eq_ignore_ascii_case("overworld") {
        return "Overworld".to_string();
    }

    let mut out = String::new();
    for (i, word) in id
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .enumerate()
    {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
        }
    }

    if out.is_empty() {
        "Unknown".to_string()
    } else {
        out
    }
}

fn tree_poi(gid: u32) -> Option<(u8, String)> {
    let (icon_index, name, level_required) = match gid {
        1263 | 1264 | 1265 | 1448 | 1449 | 1450 | 1451 | 1452 | 1809 | 1810 | 1993 => {
            (1, "Oak Tree", 1)
        }
        1690..=1692 => (3, "Willow Tree", 15),
        2147..=2154 => (4, "Maple Tree", 45),
        2155..=2162 => (5, "Yew Tree", 60),
        _ => return None,
    };

    Some((
        icon_index,
        format!("Tree, {} (Lv.{})", name, level_required),
    ))
}

fn format_display_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Unknown".to_string();
    }

    let mut out = String::new();
    for (i, word) in trimmed
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .enumerate()
    {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
        }
    }

    if out.is_empty() {
        "Unknown".to_string()
    } else {
        out
    }
}

fn sample_chunk_tiles(tiles: &[u32], sample_dim: usize) -> Vec<u32> {
    let mut sampled = Vec::with_capacity(sample_dim * sample_dim);
    let cell_span = (CHUNK_SIZE as usize / sample_dim).max(1);

    for sample_y in 0..sample_dim {
        for sample_x in 0..sample_dim {
            let start_x = sample_x * cell_span;
            let start_y = sample_y * cell_span;
            let end_x = if sample_x + 1 == sample_dim {
                CHUNK_SIZE as usize
            } else {
                ((sample_x + 1) * cell_span).min(CHUNK_SIZE as usize)
            };
            let end_y = if sample_y + 1 == sample_dim {
                CHUNK_SIZE as usize
            } else {
                ((sample_y + 1) * cell_span).min(CHUNK_SIZE as usize)
            };

            let mut counts: HashMap<u32, usize> = HashMap::new();
            for local_y in start_y..end_y {
                let row_start = local_y * CHUNK_SIZE as usize;
                for local_x in start_x..end_x {
                    let tile_id = tiles.get(row_start + local_x).copied().unwrap_or(0);
                    if tile_id != 0 {
                        *counts.entry(tile_id).or_insert(0) += 1;
                    }
                }
            }

            let fallback_x =
                (start_x + end_x.saturating_sub(start_x) / 2).min(CHUNK_SIZE as usize - 1);
            let fallback_y =
                (start_y + end_y.saturating_sub(start_y) / 2).min(CHUNK_SIZE as usize - 1);
            let fallback = tiles
                .get(fallback_y * CHUNK_SIZE as usize + fallback_x)
                .copied()
                .unwrap_or(0);

            let dominant = counts
                .into_iter()
                .max_by(|(tile_a, count_a), (tile_b, count_b)| {
                    count_a.cmp(count_b).then_with(|| tile_b.cmp(tile_a))
                })
                .map(|(tile_id, _)| tile_id)
                .unwrap_or(fallback);

            sampled.push(dominant);
        }
    }

    sampled
}

pub(super) async fn build_overworld_world_map(
    world: &Arc<World>,
    chunk_coords: &[ChunkCoord],
    npcs: &HashMap<String, Npc>,
    chest_registry: &crate::chest::ChestRegistry,
    overworld_chest_spawns: &[crate::chest::ChestSpawn],
    waystone_manager: &crate::waystone::WaystoneManager,
) -> ServerMessage {
    let mut chunks = Vec::with_capacity(chunk_coords.len());
    let mut pois = Vec::new();

    let mut min_world_x = i32::MAX;
    let mut min_world_y = i32::MAX;
    let mut max_world_x = i32::MIN;
    let mut max_world_y = i32::MIN;

    for coord in chunk_coords {
        min_world_x = min_world_x.min(coord.x * CHUNK_SIZE as i32);
        min_world_y = min_world_y.min(coord.y * CHUNK_SIZE as i32);
        max_world_x = max_world_x.max((coord.x + 1) * CHUNK_SIZE as i32);
        max_world_y = max_world_y.max((coord.y + 1) * CHUNK_SIZE as i32);

        let Some(chunk) = world.get_chunk_data(*coord).await else {
            continue;
        };
        let Some(ground_layer) = chunk
            .layers
            .iter()
            .find(|layer| layer.layer_type == ChunkLayerType::Ground)
        else {
            continue;
        };

        chunks.push(WorldMapChunkSampleData {
            chunk_x: coord.x,
            chunk_y: coord.y,
            low_tiles: sample_chunk_tiles(&ground_layer.tiles, WORLD_MAP_LOW_SAMPLE_DIM),
            high_tiles: sample_chunk_tiles(&ground_layer.tiles, WORLD_MAP_HIGH_SAMPLE_DIM),
        });

        let base_x = coord.x * CHUNK_SIZE as i32;
        let base_y = coord.y * CHUNK_SIZE as i32;
        for portal in &chunk.portals {
            let x = base_x as f32 + portal.x as f32 + portal.width.max(1) as f32 * 0.5;
            let y = base_y as f32 + portal.y as f32 + portal.height.max(1) as f32 * 0.5;
            pois.push(WorldMapPoiData {
                x,
                y,
                label: format!("Teleport, {}", format_map_display_name(&portal.target_map)),
                icon_index: 7,
                kind: WORLD_MAP_POI_TELEPORT,
            });
        }
        for obj in &chunk.objects {
            if let Some((icon_index, label)) = tree_poi(obj.gid) {
                pois.push(WorldMapPoiData {
                    x: obj.tile_x as f32 + 0.5,
                    y: obj.tile_y as f32 + 0.5,
                    label,
                    icon_index,
                    kind: WORLD_MAP_POI_TREE,
                });
            }
        }
    }

    for chest_spawn in overworld_chest_spawns {
        let chest_name = chest_registry
            .get(&chest_spawn.chest_id)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "Chest".to_string());
        pois.push(WorldMapPoiData {
            x: chest_spawn.x as f32 + 0.5,
            y: chest_spawn.y as f32 + 0.5,
            label: format!("Chest, {}", chest_name),
            icon_index: 9,
            kind: WORLD_MAP_POI_CHEST,
        });
    }

    for waystone in waystone_manager.iter() {
        pois.push(WorldMapPoiData {
            x: waystone.x as f32 + 0.5,
            y: waystone.y as f32 + 0.5,
            label: format!("Waystone, {}", waystone.name),
            icon_index: 7,
            kind: WORLD_MAP_POI_TELEPORT,
        });
    }

    for npc in npcs.values() {
        if npc.is_banker() {
            pois.push(WorldMapPoiData {
                x: npc.x as f32,
                y: npc.y as f32,
                label: format!("Bank, {}", npc.stats.display_name),
                icon_index: 255,
                kind: WORLD_MAP_POI_SERVICE,
            });
        } else if npc.is_altar() {
            pois.push(WorldMapPoiData {
                x: npc.x as f32,
                y: npc.y as f32,
                label: format!("Altar, {}", npc.stats.display_name),
                icon_index: 255,
                kind: WORLD_MAP_POI_SERVICE,
            });
        } else if let Some(station_type) = npc.station_type() {
            pois.push(WorldMapPoiData {
                x: npc.x as f32,
                y: npc.y as f32,
                label: format!(
                    "{}, {}",
                    format_display_name(station_type),
                    npc.stats.display_name
                ),
                icon_index: 255,
                kind: WORLD_MAP_POI_SERVICE,
            });
        } else if npc.is_quest_giver() {
            pois.push(WorldMapPoiData {
                x: npc.x as f32,
                y: npc.y as f32,
                label: format!("Quest, {}", npc.stats.display_name),
                icon_index: 6,
                kind: WORLD_MAP_POI_QUEST,
            });
        }
    }

    if min_world_x == i32::MAX {
        min_world_x = 0;
        min_world_y = 0;
        max_world_x = CHUNK_SIZE as i32;
        max_world_y = CHUNK_SIZE as i32;
    }

    ServerMessage::WorldMapData {
        min_x: min_world_x,
        min_y: min_world_y,
        max_x: max_world_x,
        max_y: max_world_y,
        low_sample_dim: WORLD_MAP_LOW_SAMPLE_DIM as u8,
        high_sample_dim: WORLD_MAP_HIGH_SAMPLE_DIM as u8,
        chunks,
        pois,
    }
}

impl GameRoom {
    pub async fn get_world_map_message(&self) -> ServerMessage {
        self.overworld_world_map.clone()
    }
}
