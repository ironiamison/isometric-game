use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "itemDropped" => {
            if let Some(value) = data {
                let id = extract_string(value, "id").unwrap_or_default();
                let item_id =
                    extract_string(value, "item_id").unwrap_or_else(|| "unknown".to_string());
                let x = extract_f32(value, "x").unwrap_or(0.0);
                let y = extract_f32(value, "y").unwrap_or(0.0);
                let quantity = extract_i32(value, "quantity").unwrap_or(1);

                log::debug!("Item dropped: {} ({}) at ({}, {})", id, item_id, x, y);

                let item = if item_id == "gold" {
                    GroundItem::new_gold(id.clone(), x, y, quantity)
                } else {
                    GroundItem::new(id.clone(), item_id, x, y, quantity)
                };

                // Check if there's a dying NPC near this drop location
                let near_dying_npc = state.npcs.values().any(|npc| {
                    let dx = npc.x - x;
                    let dy = npc.y - y;
                    let dist_sq = dx * dx + dy * dy;
                    npc.is_dying() && dist_sq < 2.0 // Within ~1.4 tiles
                });

                if near_dying_npc {
                    // Delay item appearance by 0.6s to let death animation complete
                    let spawn_time = macroquad::time::get_time() + 0.6;
                    state.pending_ground_items.push((item, spawn_time));
                } else {
                    // Spawn immediately (player drop, etc.)
                    state.ground_items.insert(id, item);
                }
            }
        }
        "itemPickedUp" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let player_id = extract_string(value, "player_id").unwrap_or_default();

                log::debug!("Item {} picked up by {}", item_id, player_id);
                state.ground_items.remove(&item_id);
                state
                    .pending_ground_items
                    .retain(|(item, _)| item.id != item_id);
            }
        }
        "itemDespawned" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::debug!("Item {} despawned", item_id);
                state.ground_items.remove(&item_id);
                state
                    .pending_ground_items
                    .retain(|(item, _)| item.id != item_id);
            }
        }
        "itemQuantityUpdated" => {
            if let Some(value) = data {
                let id = extract_string(value, "id").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(1);

                log::debug!("Item {} quantity updated to {}", id, quantity);

                if let Some(item) = state.ground_items.get_mut(&id) {
                    item.quantity = quantity;
                    // Regenerate gold pile with new quantity
                    if item.item_id == "gold" {
                        item.gold_pile = Some(crate::game::item::GoldPileState::new(
                            quantity,
                            macroquad::time::get_time(),
                        ));
                    }
                } else {
                    for pending in &mut state.pending_ground_items {
                        if pending.0.id == id {
                            pending.0.quantity = quantity;
                            if pending.0.item_id == "gold" {
                                pending.0.gold_pile = Some(crate::game::item::GoldPileState::new(
                                    quantity,
                                    macroquad::time::get_time(),
                                ));
                            }
                            break;
                        }
                    }
                }
            }
        }
        "inventoryUpdate" => {
            if let Some(value) = data {
                handle_inventory_update(value, state);
            }
        }
        "itemUsed" => {
            // Server sends this only to the owning player (unicast)
            if let Some(value) = data {
                let slot = extract_u8(value, "slot").unwrap_or(0);
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let effect = extract_string(value, "effect").unwrap_or_default();
                log::debug!(
                    "Item used: slot {} item {} effect {}",
                    slot,
                    item_id,
                    effect
                );
            }
        }
        "chunkData" => {
            if let Some(value) = data {
                let chunk_x = extract_i32(value, "chunkX").unwrap_or(0);
                let chunk_y = extract_i32(value, "chunkY").unwrap_or(0);

                // Parse layers array
                let mut layers: Vec<(u8, Vec<u32>)> = Vec::new();
                if let Some(layers_arr) = extract_array(value, "layers") {
                    for layer_value in layers_arr {
                        let layer_type = extract_u8(layer_value, "layerType").unwrap_or(0);
                        let tiles: Vec<u32> = extract_array(layer_value, "tiles")
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|u| u as u32))
                                    .collect()
                            })
                            .unwrap_or_default();
                        layers.push((layer_type, tiles));
                    }
                }

                // Parse collision bytes
                let collision: Vec<u8> = extract_array(value, "collision")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|u| u as u8))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse map objects
                let mut objects: Vec<MapObject> = Vec::new();
                if let Some(objects_arr) = extract_array(value, "objects") {
                    for obj_value in objects_arr {
                        let gid = obj_value["gid"].as_u64().unwrap_or(0) as u32;
                        let tile_x = obj_value["tileX"].as_i64().unwrap_or(0) as i32;
                        let tile_y = obj_value["tileY"].as_i64().unwrap_or(0) as i32;
                        let width = obj_value["width"].as_u64().unwrap_or(0) as u32;
                        let height = obj_value["height"].as_u64().unwrap_or(0) as u32;
                        objects.push(MapObject {
                            gid,
                            tile_x,
                            tile_y,
                            width,
                            height,
                        });
                    }
                }

                // Parse walls from server message
                let mut walls: Vec<Wall> = Vec::new();
                if let Some(walls_arr) = extract_array(value, "walls") {
                    for w in walls_arr {
                        let gid = w["gid"].as_u64().unwrap_or(0) as u32;
                        let tile_x = w["tileX"].as_i64().unwrap_or(0) as i32;
                        let tile_y = w["tileY"].as_i64().unwrap_or(0) as i32;
                        let edge_str = w["edge"].as_str().unwrap_or("down");
                        let edge = match edge_str {
                            "right" => WallEdge::Right,
                            _ => WallEdge::Down,
                        };
                        walls.push(Wall {
                            gid,
                            tile_x,
                            tile_y,
                            edge,
                        });
                    }
                }

                // Parse portals from server message
                let mut portals: Vec<Portal> = Vec::new();
                if let Some(portals_arr) = extract_array(value, "portals") {
                    for p in portals_arr {
                        let id = extract_string(p, "id").unwrap_or_default();
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        let width = extract_i32(p, "width").unwrap_or(1);
                        let height = extract_i32(p, "height").unwrap_or(1);
                        let target_map = extract_string(p, "targetMap").unwrap_or_default();
                        let target_spawn = extract_string(p, "targetSpawn").unwrap_or_default();
                        portals.push(Portal {
                            id,
                            x,
                            y,
                            width,
                            height,
                            target_map,
                            target_spawn,
                        });
                    }
                }

                // Parse optional heightmap data
                let heightmap: Option<Vec<u8>> = extract_array(value, "heightmap").map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_i64().map(|i| i as u8))
                        .collect()
                });
                let block_types_down: Option<Vec<u16>> = extract_array(value, "blockTypesDown")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_i64().map(|i| i as u16))
                            .collect()
                    });
                let block_types_right: Option<Vec<u16>> = extract_array(value, "blockTypesRight")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_i64().map(|i| i as u16))
                            .collect()
                    });

                log::debug!("Received chunk data: ({}, {}) with {} layers, {} collision bytes, {} objects, {} walls, {} portals",
                    chunk_x, chunk_y, layers.len(), collision.len(), objects.len(), walls.len(), portals.len());

                state.chunk_manager.load_chunk(
                    chunk_x,
                    chunk_y,
                    layers,
                    &collision,
                    objects,
                    walls,
                    portals,
                    heightmap,
                    block_types_down,
                    block_types_right,
                );
            }
        }
        "chunkNotFound" => {
            if let Some(value) = data {
                let chunk_x = extract_i32(value, "chunkX").unwrap_or(0);
                let chunk_y = extract_i32(value, "chunkY").unwrap_or(0);
                log::warn!("Chunk not found: ({}, {})", chunk_x, chunk_y);
            }
        }

        // ========== Quest System Messages ==========
        _ => return false,
    }
    true
}
