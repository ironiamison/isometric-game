use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::ChunkData {
            chunk_x,
            chunk_y,
            layers,
            collision,
            objects,
            walls,
            portals,
            heightmap,
            block_types_down,
            block_types_right,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chunkX".into()),
                Value::Integer((*chunk_x as i64).into()),
            ));
            map.push((
                Value::String("chunkY".into()),
                Value::Integer((*chunk_y as i64).into()),
            ));

            // Encode layers
            let layer_values: Vec<Value> = layers
                .iter()
                .map(|l| {
                    let mut lmap = Vec::new();
                    lmap.push((
                        Value::String("layerType".into()),
                        Value::Integer((l.layer_type as i64).into()),
                    ));
                    let tiles: Vec<Value> = l
                        .tiles
                        .iter()
                        .map(|&t| Value::Integer((t as i64).into()))
                        .collect();
                    lmap.push((Value::String("tiles".into()), Value::Array(tiles)));
                    Value::Map(lmap)
                })
                .collect();
            map.push((Value::String("layers".into()), Value::Array(layer_values)));

            // Encode collision as binary
            let collision_bytes: Vec<Value> = collision
                .iter()
                .map(|&b| Value::Integer((b as i64).into()))
                .collect();
            map.push((
                Value::String("collision".into()),
                Value::Array(collision_bytes),
            ));

            // Encode map objects
            let object_values: Vec<Value> = objects
                .iter()
                .map(|o| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("gid".into()),
                        Value::Integer((o.gid as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileX".into()),
                        Value::Integer((o.tile_x as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileY".into()),
                        Value::Integer((o.tile_y as i64).into()),
                    ));
                    omap.push((
                        Value::String("width".into()),
                        Value::Integer((o.width as i64).into()),
                    ));
                    omap.push((
                        Value::String("height".into()),
                        Value::Integer((o.height as i64).into()),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objects".into()), Value::Array(object_values)));

            // Encode walls
            let wall_values: Vec<Value> = walls
                .iter()
                .map(|w| {
                    let mut wmap = Vec::new();
                    wmap.push((
                        Value::String("gid".into()),
                        Value::Integer((w.gid as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileX".into()),
                        Value::Integer((w.tile_x as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileY".into()),
                        Value::Integer((w.tile_y as i64).into()),
                    ));
                    wmap.push((
                        Value::String("edge".into()),
                        Value::String(w.edge.clone().into()),
                    ));
                    Value::Map(wmap)
                })
                .collect();
            map.push((Value::String("walls".into()), Value::Array(wall_values)));

            // Encode portals
            let portal_values: Vec<Value> = portals
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("id".into()),
                        Value::String(p.id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("width".into()),
                        Value::Integer((p.width as i64).into()),
                    ));
                    pmap.push((
                        Value::String("height".into()),
                        Value::Integer((p.height as i64).into()),
                    ));
                    pmap.push((
                        Value::String("targetMap".into()),
                        Value::String(p.target_map.clone().into()),
                    ));
                    pmap.push((
                        Value::String("targetSpawn".into()),
                        Value::String(p.target_spawn.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("portals".into()), Value::Array(portal_values)));

            // Encode optional heightmap data
            if let Some(hm) = heightmap {
                let hm_values: Vec<Value> = hm
                    .iter()
                    .map(|&h| Value::Integer((h as i64).into()))
                    .collect();
                map.push((Value::String("heightmap".into()), Value::Array(hm_values)));
            }
            if let Some(bt) = block_types_down {
                let bt_values: Vec<Value> = bt
                    .iter()
                    .map(|&b| Value::Integer((b as i64).into()))
                    .collect();
                map.push((
                    Value::String("blockTypesDown".into()),
                    Value::Array(bt_values),
                ));
            }
            if let Some(bt) = block_types_right {
                let bt_values: Vec<Value> = bt
                    .iter()
                    .map(|&b| Value::Integer((b as i64).into()))
                    .collect();
                map.push((
                    Value::String("blockTypesRight".into()),
                    Value::Array(bt_values),
                ));
            }

            Value::Map(map)
        }
        ServerMessage::ChunkNotFound { chunk_x, chunk_y } => {
            let mut map = Vec::new();
            map.push((
                Value::String("chunkX".into()),
                Value::Integer((*chunk_x as i64).into()),
            ));
            map.push((
                Value::String("chunkY".into()),
                Value::Integer((*chunk_y as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::EntityDefinitions { entities } => {
            let mut map = Vec::new();
            let entity_values: Vec<Value> = entities
                .iter()
                .map(|e| {
                    let mut emap = Vec::new();
                    emap.push((
                        Value::String("id".into()),
                        Value::String(e.id.clone().into()),
                    ));
                    emap.push((
                        Value::String("displayName".into()),
                        Value::String(e.display_name.clone().into()),
                    ));
                    emap.push((
                        Value::String("sprite".into()),
                        Value::String(e.sprite.clone().into()),
                    ));
                    emap.push((
                        Value::String("animationType".into()),
                        Value::String(e.animation_type.clone().into()),
                    ));
                    emap.push((
                        Value::String("maxHp".into()),
                        Value::Integer((e.max_hp as i64).into()),
                    ));
                    Value::Map(emap)
                })
                .collect();
            map.push((
                Value::String("entities".into()),
                Value::Array(entity_values),
            ));
            Value::Map(map)
        }
        ServerMessage::MapTransition {
            map_type,
            map_id,
            spawn_x,
            spawn_y,
            instance_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("mapType".into()),
                Value::String(map_type.clone().into()),
            ));
            map.push((
                Value::String("mapId".into()),
                Value::String(map_id.clone().into()),
            ));
            map.push((Value::String("spawnX".into()), Value::F64(*spawn_x as f64)));
            map.push((Value::String("spawnY".into()), Value::F64(*spawn_y as f64)));
            map.push((
                Value::String("instanceId".into()),
                Value::String(instance_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::InteriorData {
            map_id,
            name,
            instance_id,
            width,
            height,
            spawn_x,
            spawn_y,
            layers,
            collision,
            portals,
            objects,
            walls,
            heightmap,
            block_types_down,
            block_types_right,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("mapId".into()),
                Value::String(map_id.clone().into()),
            ));
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));
            map.push((
                Value::String("instanceId".into()),
                Value::String(instance_id.clone().into()),
            ));
            map.push((
                Value::String("width".into()),
                Value::Integer((*width as i64).into()),
            ));
            map.push((
                Value::String("height".into()),
                Value::Integer((*height as i64).into()),
            ));
            map.push((Value::String("spawnX".into()), Value::F64(*spawn_x as f64)));
            map.push((Value::String("spawnY".into()), Value::F64(*spawn_y as f64)));

            // Encode layers (same format as ChunkData)
            let layer_values: Vec<Value> = layers
                .iter()
                .map(|l| {
                    let mut lmap = Vec::new();
                    lmap.push((
                        Value::String("layerType".into()),
                        Value::Integer((l.layer_type as i64).into()),
                    ));
                    let tiles: Vec<Value> = l
                        .tiles
                        .iter()
                        .map(|&t| Value::Integer((t as i64).into()))
                        .collect();
                    lmap.push((Value::String("tiles".into()), Value::Array(tiles)));
                    Value::Map(lmap)
                })
                .collect();
            map.push((Value::String("layers".into()), Value::Array(layer_values)));

            // Encode collision as binary array
            let collision_bytes: Vec<Value> = collision
                .iter()
                .map(|&b| Value::Integer((b as i64).into()))
                .collect();
            map.push((
                Value::String("collision".into()),
                Value::Array(collision_bytes),
            ));

            // Encode portals
            let portal_values: Vec<Value> = portals
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("id".into()),
                        Value::String(p.id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("width".into()),
                        Value::Integer((p.width as i64).into()),
                    ));
                    pmap.push((
                        Value::String("height".into()),
                        Value::Integer((p.height as i64).into()),
                    ));
                    pmap.push((
                        Value::String("targetMap".into()),
                        Value::String(p.target_map.clone().into()),
                    ));
                    pmap.push((
                        Value::String("targetSpawn".into()),
                        Value::String(p.target_spawn.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("portals".into()), Value::Array(portal_values)));

            // Encode objects (trees, rocks, decorations)
            let object_values: Vec<Value> = objects
                .iter()
                .map(|o| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("gid".into()),
                        Value::Integer((o.gid as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileX".into()),
                        Value::Integer((o.tile_x as i64).into()),
                    ));
                    omap.push((
                        Value::String("tileY".into()),
                        Value::Integer((o.tile_y as i64).into()),
                    ));
                    omap.push((
                        Value::String("width".into()),
                        Value::Integer((o.width as i64).into()),
                    ));
                    omap.push((
                        Value::String("height".into()),
                        Value::Integer((o.height as i64).into()),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objects".into()), Value::Array(object_values)));

            // Encode walls
            let wall_values: Vec<Value> = walls
                .iter()
                .map(|w| {
                    let mut wmap = Vec::new();
                    wmap.push((
                        Value::String("gid".into()),
                        Value::Integer((w.gid as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileX".into()),
                        Value::Integer((w.tile_x as i64).into()),
                    ));
                    wmap.push((
                        Value::String("tileY".into()),
                        Value::Integer((w.tile_y as i64).into()),
                    ));
                    wmap.push((
                        Value::String("edge".into()),
                        Value::String(w.edge.clone().into()),
                    ));
                    Value::Map(wmap)
                })
                .collect();
            map.push((Value::String("walls".into()), Value::Array(wall_values)));

            // Encode optional heightmap
            if let Some(hm) = heightmap {
                let hm_values: Vec<Value> = hm
                    .iter()
                    .map(|&h| Value::Integer((h as i64).into()))
                    .collect();
                map.push((Value::String("heightmap".into()), Value::Array(hm_values)));
            }
            if let Some(btd) = block_types_down {
                let btd_values: Vec<Value> = btd
                    .iter()
                    .map(|&b| Value::Integer((b as i64).into()))
                    .collect();
                map.push((
                    Value::String("blockTypesDown".into()),
                    Value::Array(btd_values),
                ));
            }
            if let Some(btr) = block_types_right {
                let btr_values: Vec<Value> = btr
                    .iter()
                    .map(|&b| Value::Integer((b as i64).into()))
                    .collect();
                map.push((
                    Value::String("blockTypesRight".into()),
                    Value::Array(btr_values),
                ));
            }

            Value::Map(map)
        }
        ServerMessage::GatheringMarkers { markers } => {
            let mut map = Vec::new();
            let marker_values: Vec<Value> = markers
                .iter()
                .map(|m| {
                    let mut mmap = Vec::new();
                    mmap.push((
                        Value::String("x".into()),
                        Value::Integer((m.x as i64).into()),
                    ));
                    mmap.push((
                        Value::String("y".into()),
                        Value::Integer((m.y as i64).into()),
                    ));
                    mmap.push((
                        Value::String("zone_id".into()),
                        Value::String(m.zone_id.clone().into()),
                    ));
                    mmap.push((
                        Value::String("skill".into()),
                        Value::String(m.skill.clone().into()),
                    ));
                    Value::Map(mmap)
                })
                .collect();
            map.push((Value::String("markers".into()), Value::Array(marker_values)));
            Value::Map(map)
        }
        ServerMessage::ChairPositions { positions } => {
            let mut map = Vec::new();
            let pos_values: Vec<Value> = positions
                .iter()
                .map(|(x, y)| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((*x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((*y as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("positions".into()), Value::Array(pos_values)));
            Value::Map(map)
        }
        ServerMessage::ChestPositions { positions } => {
            let mut map = Vec::new();
            let pos_values: Vec<Value> = positions
                .iter()
                .map(|(x, y)| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((*x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((*y as i64).into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("positions".into()), Value::Array(pos_values)));
            Value::Map(map)
        }
        ServerMessage::WorldMapData {
            min_x,
            min_y,
            max_x,
            max_y,
            low_sample_dim,
            high_sample_dim,
            chunks,
            pois,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("minX".into()),
                Value::Integer((*min_x as i64).into()),
            ));
            map.push((
                Value::String("minY".into()),
                Value::Integer((*min_y as i64).into()),
            ));
            map.push((
                Value::String("maxX".into()),
                Value::Integer((*max_x as i64).into()),
            ));
            map.push((
                Value::String("maxY".into()),
                Value::Integer((*max_y as i64).into()),
            ));
            map.push((
                Value::String("lowSampleDim".into()),
                Value::Integer((*low_sample_dim as i64).into()),
            ));
            map.push((
                Value::String("highSampleDim".into()),
                Value::Integer((*high_sample_dim as i64).into()),
            ));
            map.push((
                Value::String("chunks".into()),
                Value::Array(
                    chunks
                        .iter()
                        .map(|chunk| {
                            Value::Map(vec![
                                (
                                    Value::String("chunkX".into()),
                                    Value::Integer((chunk.chunk_x as i64).into()),
                                ),
                                (
                                    Value::String("chunkY".into()),
                                    Value::Integer((chunk.chunk_y as i64).into()),
                                ),
                                (
                                    Value::String("lowTiles".into()),
                                    Value::Array(
                                        chunk
                                            .low_tiles
                                            .iter()
                                            .map(|tile| Value::Integer((*tile as i64).into()))
                                            .collect(),
                                    ),
                                ),
                                (
                                    Value::String("highTiles".into()),
                                    Value::Array(
                                        chunk
                                            .high_tiles
                                            .iter()
                                            .map(|tile| Value::Integer((*tile as i64).into()))
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ));
            map.push((
                Value::String("pois".into()),
                Value::Array(
                    pois.iter()
                        .map(|poi| {
                            Value::Map(vec![
                                (Value::String("x".into()), Value::F32(poi.x)),
                                (Value::String("y".into()), Value::F32(poi.y)),
                                (
                                    Value::String("label".into()),
                                    Value::String(poi.label.clone().into()),
                                ),
                                (
                                    Value::String("iconIndex".into()),
                                    Value::Integer((poi.icon_index as i64).into()),
                                ),
                                (
                                    Value::String("kind".into()),
                                    Value::Integer((poi.kind as i64).into()),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ));
            Value::Map(map)
        }
        ServerMessage::SitResult {
            success,
            tile_x,
            tile_y,
            direction,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("tileX".into()),
                Value::Integer((*tile_x as i64).into()),
            ));
            map.push((
                Value::String("tileY".into()),
                Value::Integer((*tile_y as i64).into()),
            ));
            map.push((
                Value::String("direction".into()),
                Value::Integer((*direction as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::FarmingPatchStates {
            patches,
            unlocked_plots,
            tile_overrides,
        } => {
            let mut map = Vec::new();
            let patch_values: Vec<Value> = patches
                .iter()
                .map(|p| {
                    let mut pmap = Vec::new();
                    pmap.push((
                        Value::String("patch_id".into()),
                        Value::String(p.patch_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("x".into()),
                        Value::Integer((p.x as i64).into()),
                    ));
                    pmap.push((
                        Value::String("y".into()),
                        Value::Integer((p.y as i64).into()),
                    ));
                    pmap.push((
                        Value::String("state".into()),
                        Value::String(p.state.clone().into()),
                    ));
                    pmap.push((
                        Value::String("crop_id".into()),
                        Value::String(p.crop_id.clone().into()),
                    ));
                    pmap.push((
                        Value::String("growth_stage".into()),
                        Value::Integer((p.growth_stage as i64).into()),
                    ));
                    pmap.push((
                        Value::String("owner_id".into()),
                        Value::String(p.owner_id.clone().into()),
                    ));
                    Value::Map(pmap)
                })
                .collect();
            map.push((Value::String("patches".into()), Value::Array(patch_values)));
            let plot_values: Vec<Value> = unlocked_plots
                .iter()
                .map(|p| Value::Integer((*p as i64).into()))
                .collect();
            map.push((
                Value::String("unlocked_plots".into()),
                Value::Array(plot_values),
            ));
            let tile_override_values: Vec<Value> = tile_overrides
                .iter()
                .map(|t| {
                    let mut tmap = Vec::new();
                    tmap.push((
                        Value::String("x".into()),
                        Value::Integer((t.x as i64).into()),
                    ));
                    tmap.push((
                        Value::String("y".into()),
                        Value::Integer((t.y as i64).into()),
                    ));
                    tmap.push((
                        Value::String("tile_id".into()),
                        Value::Integer((t.tile_id as i64).into()),
                    ));
                    Value::Map(tmap)
                })
                .collect();
            map.push((
                Value::String("tile_overrides".into()),
                Value::Array(tile_override_values),
            ));
            Value::Map(map)
        }
        ServerMessage::PatchStateUpdate {
            patch_id,
            state,
            crop_id,
            growth_stage,
            owner_id,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("patch_id".into()),
                Value::String(patch_id.clone().into()),
            ));
            map.push((
                Value::String("state".into()),
                Value::String(state.clone().into()),
            ));
            map.push((
                Value::String("crop_id".into()),
                Value::String(crop_id.clone().into()),
            ));
            map.push((
                Value::String("growth_stage".into()),
                Value::Integer((*growth_stage as i64).into()),
            ));
            map.push((
                Value::String("owner_id".into()),
                Value::String(owner_id.clone().into()),
            ));
            Value::Map(map)
        }
        // Friend system messages
        _ => return None,
    };
    Some(value)
}
