use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "mapTransition" => {
            if let Some(value) = data {
                let map_type = extract_string(value, "mapType").unwrap_or_default();
                let map_id = extract_string(value, "mapId").unwrap_or_default();
                let spawn_x = extract_f32(value, "spawnX").unwrap_or(0.0);
                let spawn_y = extract_f32(value, "spawnY").unwrap_or(0.0);
                let instance_id = extract_string(value, "instanceId").unwrap_or_default();

                if map_type == "overworld" {
                    // Returning to overworld from interior

                    // Switch back to overworld music
                    state.pending_music = Some("assets/audio/start.ogg".to_string());

                    // Trigger area banner for overworld
                    state.area_banner.show(OVERWORLD_NAME);

                    // Clear interior mode
                    state.chunk_manager.clear_interior();
                    state.current_interior = None;
                    state.current_instance = None;

                    // Clear interior NPCs and ground items (will be repopulated by stateSync)
                    state.npcs.clear();
                    state.ground_items.clear();

                    // Clear boss fight state
                    state.boss = None;
                    state.aoe_warnings.clear();
                    state.explosions.clear();

                    // Reset portal check and ignore the spawn tile until player steps off
                    state.last_portal_check_pos = None;
                    let spawn_tile = (spawn_x.floor() as i32, spawn_y.floor() as i32);
                    state.portal_ignore_tile = Some(spawn_tile);

                    // Update player position (both visual and server-authoritative)
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get_mut(local_id) {
                            player.x = spawn_x;
                            player.y = spawn_y;
                            player.server_x = spawn_x;
                            player.server_y = spawn_y;
                            player.target_x = spawn_x;
                            player.target_y = spawn_y;
                            player.vel_x = 0.0;
                            player.vel_y = 0.0;
                        }
                    }

                    // Start fade-in transition directly (no loading needed)
                    state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    state.map_transition.progress = 1.0;
                } else {
                    // Switch to boss music if entering boss cave
                    if map_id.contains("desert_boss_cave") {
                        state.pending_music =
                            Some("assets/audio/desert-boss-battle.ogg".to_string());
                    }

                    // Transitioning to interior - wait for interiorData
                    state.start_transition(map_type, map_id, spawn_x, spawn_y, instance_id);
                }
            }
        }
        "interiorData" => {
            if let Some(value) = data {
                let map_id = extract_string(value, "mapId").unwrap_or_default();
                let instance_id = extract_string(value, "instanceId").unwrap_or_default();
                let width = extract_u32(value, "width").unwrap_or(32);
                let height = extract_u32(value, "height").unwrap_or(32);
                let spawn_x = extract_f32(value, "spawnX").unwrap_or(0.0);
                let spawn_y = extract_f32(value, "spawnY").unwrap_or(0.0);

                // Extract interior name (fallback to map_id if missing)
                let name = extract_string(value, "name").unwrap_or(map_id.clone());

                // Trigger area banner
                state.area_banner.show(&name);

                // Parse layers
                let mut layers: Vec<(u8, Vec<u32>)> = Vec::new();
                if let Some(layers_arr) = extract_array(value, "layers") {
                    for layer_data in layers_arr {
                        let layer_type = extract_u8(layer_data, "layerType").unwrap_or(0);
                        let tiles: Vec<u32> = extract_array(layer_data, "tiles")
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u32))
                                    .collect()
                            })
                            .unwrap_or_default();
                        layers.push((layer_type, tiles));
                    }
                }

                // Parse collision
                let collision: Vec<u8> = extract_array(value, "collision")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse portals
                let mut portals: Vec<Portal> = Vec::new();
                if let Some(portals_arr) = extract_array(value, "portals") {
                    for p in portals_arr {
                        portals.push(Portal {
                            id: extract_string(p, "id").unwrap_or_default(),
                            x: extract_i32(p, "x").unwrap_or(0),
                            y: extract_i32(p, "y").unwrap_or(0),
                            width: extract_i32(p, "width").unwrap_or(1),
                            height: extract_i32(p, "height").unwrap_or(1),
                            target_map: extract_string(p, "targetMap").unwrap_or_default(),
                            target_spawn: extract_string(p, "targetSpawn").unwrap_or_default(),
                        });
                    }
                }

                // Parse objects (trees, rocks, decorations)
                let mut objects: Vec<MapObject> = Vec::new();
                if let Some(objects_arr) = extract_array(value, "objects") {
                    for o in objects_arr {
                        objects.push(MapObject {
                            gid: extract_u32(o, "gid").unwrap_or(0),
                            tile_x: extract_i32(o, "tileX").unwrap_or(0),
                            tile_y: extract_i32(o, "tileY").unwrap_or(0),
                            width: extract_u32(o, "width").unwrap_or(32),
                            height: extract_u32(o, "height").unwrap_or(32),
                        });
                    }
                }

                // Parse walls
                let mut walls: Vec<Wall> = Vec::new();
                if let Some(walls_arr) = extract_array(value, "walls") {
                    for w in walls_arr {
                        let edge_str = extract_string(w, "edge").unwrap_or_default();
                        let edge = match edge_str.as_str() {
                            "right" | "Right" => WallEdge::Right,
                            _ => WallEdge::Down,
                        };
                        walls.push(Wall {
                            gid: extract_u32(w, "gid").unwrap_or(0),
                            tile_x: extract_i32(w, "tileX").unwrap_or(0),
                            tile_y: extract_i32(w, "tileY").unwrap_or(0),
                            edge,
                        });
                    }
                }

                // Parse optional heightmap
                let heightmap: Option<Vec<u8>> = extract_array(value, "heightmap").map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u8))
                        .collect()
                });
                let block_types_down: Option<Vec<u16>> = extract_array(value, "blockTypesDown")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u16))
                            .collect()
                    });
                let block_types_right: Option<Vec<u16>> = extract_array(value, "blockTypesRight")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u16))
                            .collect()
                    });

                // Clear world data when entering interior
                state.npcs.clear();
                state.ground_items.clear();
                state.chair_positions.clear();
                state.chest_positions.clear();
                state.gathering_markers.clear();
                state.pending_chair_sit = None;

                // Clear other players (keep only local player) to avoid ghost collisions
                if let Some(local_id) = &state.local_player_id {
                    let local_player = state.players.remove(local_id);
                    state.players.clear();
                    if let Some(player) = local_player {
                        state.players.insert(local_id.clone(), player);
                    }
                } else {
                    state.players.clear();
                }

                // Load the interior
                state.chunk_manager.load_interior(
                    width,
                    height,
                    layers,
                    &collision,
                    portals,
                    objects,
                    walls,
                    heightmap,
                    block_types_down,
                    block_types_right,
                );
                state.current_interior = Some(map_id.clone());
                state.current_instance = Some(instance_id);
                state.fishing_bubbles.clear();

                // Reset portal check and ignore the spawn tile until player steps off
                state.last_portal_check_pos = None;
                let spawn_tile = (spawn_x.floor() as i32, spawn_y.floor() as i32);
                state.portal_ignore_tile = Some(spawn_tile);

                // Update player position (both visual and server-authoritative)
                if let Some(local_id) = &state.local_player_id {
                    if let Some(player) = state.players.get_mut(local_id) {
                        player.x = spawn_x;
                        player.y = spawn_y;
                        player.server_x = spawn_x;
                        player.server_y = spawn_y;
                        player.target_x = spawn_x;
                        player.target_y = spawn_y;
                    }
                }

                // Complete the transition (fade in)
                // Handle both Loading (normal case) and FadingOut (data arrived quickly)
                match state.map_transition.state {
                    crate::game::state::TransitionState::Loading => {
                        state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    }
                    crate::game::state::TransitionState::FadingOut => {
                        // Data arrived before fade out completed - skip to fade in
                        state.map_transition.progress = 1.0;
                        state.map_transition.state = crate::game::state::TransitionState::FadingIn;
                    }
                    _ => {}
                }
            }
        }
        "chairPositions" => {
            if let Some(value) = data {
                if let Some(positions_arr) = extract_array(value, "positions") {
                    let mut positions = Vec::new();
                    for p in positions_arr {
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        positions.push((x, y));
                    }
                    log::info!("Received {} chair positions", positions.len());
                    state.chair_positions = positions;
                }
            }
        }
        "chestPositions" => {
            if let Some(value) = data {
                if let Some(positions_arr) = extract_array(value, "positions") {
                    let mut positions = Vec::new();
                    for p in positions_arr {
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        positions.push((x, y));
                    }
                    log::info!("Received {} chest positions", positions.len());
                    state.chest_positions = positions;
                }
            }
        }
        "sitResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                if success {
                    let tile_x = extract_i32(value, "tileX").unwrap_or(0);
                    let tile_y = extract_i32(value, "tileY").unwrap_or(0);
                    let direction = extract_i32(value, "direction").unwrap_or(0) as u8;
                    state.is_sitting = true;
                    if let Some(local_id) = &state.local_player_id {
                        if let Some(player) = state.players.get_mut(local_id) {
                            player.x = tile_x as f32;
                            player.y = tile_y as f32;
                            player.server_x = tile_x as f32;
                            player.server_y = tile_y as f32;
                            player.target_x = tile_x as f32;
                            player.target_y = tile_y as f32;
                            player.direction = Direction::from_u8(direction);
                            player.animation.direction = Direction::from_u8(direction);
                            player.sit_chair();
                        }
                    }
                }
            }
        }
        "gatheringMarkers" => {
            if let Some(value) = data {
                if let Some(markers_arr) = extract_array(value, "markers") {
                    let mut markers = Vec::new();
                    for m in markers_arr {
                        let x = extract_i32(m, "x").unwrap_or(0);
                        let y = extract_i32(m, "y").unwrap_or(0);
                        let zone_id = extract_string(m, "zone_id").unwrap_or_default();
                        let skill = extract_string(m, "skill").unwrap_or_default();
                        markers.push(GatheringMarker {
                            x,
                            y,
                            zone_id,
                            skill,
                        });
                    }
                    log::info!("Received {} gathering markers", markers.len());
                    state.gathering_markers = markers;
                }
            }
        }
        "worldMapData" => {
            if let Some(value) = data {
                let min_x = extract_i32(value, "minX").unwrap_or(0) as f32;
                let min_y = extract_i32(value, "minY").unwrap_or(0) as f32;
                let max_x = extract_i32(value, "maxX").unwrap_or(32) as f32;
                let max_y = extract_i32(value, "maxY").unwrap_or(32) as f32;
                let low_sample_dim = extract_u8(value, "lowSampleDim")
                    .or_else(|| extract_u8(value, "chunkSampleDim"))
                    .unwrap_or(4) as usize;
                let high_sample_dim =
                    extract_u8(value, "highSampleDim").unwrap_or(low_sample_dim as u8) as usize;

                let mut chunks = Vec::new();
                if let Some(chunks_arr) = extract_array(value, "chunks") {
                    for chunk in chunks_arr {
                        let chunk_x = extract_i32(chunk, "chunkX").unwrap_or(0);
                        let chunk_y = extract_i32(chunk, "chunkY").unwrap_or(0);
                        let low_tiles: Vec<u32> = extract_array(chunk, "lowTiles")
                            .or_else(|| extract_array(chunk, "tiles"))
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u32))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let high_tiles: Vec<u32> = extract_array(chunk, "highTiles")
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u32))
                                    .collect()
                            })
                            .unwrap_or_else(|| low_tiles.clone());
                        chunks.push(WorldMapChunkSample {
                            chunk_x,
                            chunk_y,
                            low_tiles,
                            high_tiles,
                        });
                    }
                }

                let mut pois = Vec::new();
                if let Some(pois_arr) = extract_array(value, "pois") {
                    for poi in pois_arr {
                        pois.push(WorldMapPoi {
                            x: extract_f32(poi, "x").unwrap_or(0.0),
                            y: extract_f32(poi, "y").unwrap_or(0.0),
                            label: extract_string(poi, "label").unwrap_or_default(),
                            icon_index: extract_u8(poi, "iconIndex").unwrap_or(255),
                            kind: extract_u8(poi, "kind").unwrap_or_else(|| {
                                match extract_u8(poi, "iconIndex").unwrap_or(255) {
                                    6 => WORLD_MAP_POI_KIND_QUEST,
                                    7 => WORLD_MAP_POI_KIND_TELEPORT,
                                    9 => WORLD_MAP_POI_KIND_CHEST,
                                    _ => WORLD_MAP_POI_KIND_TREE,
                                }
                            }),
                        });
                    }
                }

                state.world_map_snapshot = Some(WorldMapSnapshot {
                    bounds: WorldMapBounds {
                        min_x,
                        min_y,
                        max_x,
                        max_y,
                    },
                    low_sample_dim: low_sample_dim.max(1),
                    high_sample_dim: high_sample_dim.max(low_sample_dim).max(1),
                    chunks,
                    pois,
                });
            }
        }
        "gatheringStarted" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let zone_id = extract_string(value, "zone_id").unwrap_or_default();
                log::info!(
                    "Gathering started for player {} in zone {}",
                    player_id,
                    zone_id
                );
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_gathering = true;
                    player.gathering_started_at = macroquad::time::get_time();
                    player.play_attack();
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_gathering = true;
                    state.gathering_started_at = macroquad::time::get_time();
                }
            }
        }
        "gatheringResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let xp_gained = extract_i32(value, "xp_gained").unwrap_or(0) as i64;
                log::info!(
                    "Gathering result: player {} got {} (+{}xp)",
                    player_id,
                    item_id,
                    xp_gained
                );
                if state.local_player_id.as_deref() == Some(&player_id) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());
                    // Add XP event for floating text
                    if let Some(player) = state.players.get(&player_id) {
                        state.skill_xp_events.push(SkillXpEvent {
                            x: player.x,
                            y: player.y,
                            skill: "Fishing".to_string(),
                            xp_gained,
                            time: macroquad::time::get_time(),
                        });
                    }
                    // Add chat message about the catch
                    state.push_system_chat(format!("You caught a {}!", item_name));
                }
            }
        }
        "gatheringStopped" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let reason = extract_string(value, "reason").unwrap_or_default();
                log::info!("Gathering stopped for player {}: {}", player_id, reason);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_gathering = false;
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_gathering = false;
                    if reason == "inventory_full" {
                        state.push_system_chat("Your inventory is full!".to_string());
                        state.pending_sfx.push("error".to_string());
                    }
                }
            }
        }
        "buffApplied" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let buff_type = extract_string(value, "buff_type").unwrap_or_default();
                let duration = extract_u64(value, "duration").unwrap_or(30) as f64;
                log::info!(
                    "Buff {} applied to player {} for {}s",
                    buff_type,
                    player_id,
                    duration
                );
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.gathering_buff = Some(GatheringBuff {
                        buff_type,
                        start_time: macroquad::time::get_time(),
                        duration,
                    });
                }
            }
        }
        "potionBuffsSync" => {
            if let Some(value) = data {
                if let Some(buffs_arr) = extract_array(value, "buffs") {
                    let now = macroquad::time::get_time();
                    state.active_potion_buffs = buffs_arr
                        .iter()
                        .filter_map(|b| {
                            let stat = extract_string(b, "stat")?;
                            let amount = extract_i32(b, "amount").unwrap_or(0);
                            let remaining_ms = extract_u64(b, "remaining_ms").unwrap_or(0);
                            let source_item_id =
                                extract_string(b, "source_item_id").unwrap_or_default();
                            Some(ActivePotionBuff {
                                stat,
                                amount,
                                expires_at: now + (remaining_ms as f64 / 1000.0),
                                source_item_id,
                            })
                        })
                        .collect();
                }
            }
        }
        "buffExpired" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let buff_type = extract_string(value, "buff_type").unwrap_or_default();
                log::info!("Buff {} expired for player {}", buff_type, player_id);
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.gathering_buff = None;
                }
            }
        }

        // =====================================================================
        // Woodcutting Messages
        // =====================================================================
        "woodcuttingSwing" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let tree_x = extract_i32(value, "tree_x").unwrap_or(0);
                let tree_y = extract_i32(value, "tree_y").unwrap_or(0);

                // Check if the action is close enough to the local player to hear
                let is_local = state.local_player_id.as_deref() == Some(&player_id);
                let in_audio_range = is_local || {
                    state
                        .local_player_id
                        .as_ref()
                        .and_then(|id| state.players.get(id))
                        .map(|lp| {
                            let dx = (lp.x - tree_x as f32).abs();
                            let dy = (lp.y - tree_y as f32).abs();
                            dx.max(dy) <= SFX_AUDIBLE_RANGE
                        })
                        .unwrap_or(false)
                };

                // A swing is the authoritative signal that the local player is
                // woodcutting (there's no server-side "started" session message
                // for woodcutting like there is for fishing). Refresh the flag and
                // timestamp on every swing; GameState::update times it out shortly
                // after swings stop arriving.
                if is_local {
                    state.is_woodcutting = true;
                    state.woodcutting_started_at = macroquad::time::get_time();
                }

                // Server says player swung - play the swing animation. Swings are
                // server-authoritative (no local prediction), so this echo is the
                // single source of the animation for everyone, including the local
                // player.
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.play_attack();
                }
                if in_audio_range {
                    state
                        .pending_attack_sounds
                        .push(crate::game::state::AttackSoundType::Melee);

                    // Play woodcutting sound effect
                    state.pending_sfx.push("woodcut".to_string());
                }

                // Add tree shake effect
                state
                    .tree_shake_effects
                    .push(crate::game::state::TreeShakeEffect::new(tree_x, tree_y));

                // Spawn leaf particles at the top of the tree (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let tree_height = 60.0;
                    for _ in 0..3 {
                        state
                            .leaf_particles
                            .push(crate::game::state::LeafParticle::new_at_tree(
                                tree_x,
                                tree_y,
                                tree_height,
                            ));
                    }
                }
            }
        }
        "woodcuttingResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::info!("Woodcutting result: player {} got {}", player_id, item_id);

                // XP display is handled by the separate skillXp message
                // This handler just shows the item feedback

                if state.local_player_id.as_deref() == Some(player_id.as_str()) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());

                    // Add chat message about the chop
                    state.push_system_chat(format!("You chopped some {}!", item_name));
                }
            }
        }
        "treeDepleted" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let gid = extract_u32(value, "gid").unwrap_or(0);
                let respawn_delay_ms = extract_u64(value, "respawn_delay_ms").unwrap_or(7500);
                let now = macroquad::time::get_time();
                log::info!(
                    "Tree depleted at ({}, {}), respawn in {}ms",
                    x,
                    y,
                    respawn_delay_ms
                );

                // Add falling tree effect
                state
                    .falling_trees
                    .push(crate::game::state::FallingTreeEffect::new(x, y, gid));

                // Spawn a burst of leaves when tree falls (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let tree_height = 60.0;
                    for _ in 0..10 {
                        state
                            .leaf_particles
                            .push(crate::game::state::LeafParticle::new_at_tree(
                                x,
                                y,
                                tree_height,
                            ));
                    }
                }

                // Mark tree as depleted (hides the static tree, shows respawn timer)
                state.depleted_trees.insert(
                    (x, y),
                    crate::game::state::DepletedTreeInfo {
                        gid,
                        depleted_at: now,
                        respawn_at: now + (respawn_delay_ms as f64 / 1000.0),
                    },
                );
            }
        }
        "treeRespawned" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                log::info!("Tree respawned at ({}, {})", x, y);
                state.depleted_trees.remove(&(x, y));
            }
        }
        "depletedTreesSync" => {
            if let Some(value) = data {
                if let Some(trees_arr) = extract_array(value, "trees") {
                    state.depleted_trees.clear();
                    let now = macroquad::time::get_time();
                    for tree in trees_arr {
                        let x = extract_i32(tree, "x").unwrap_or(0);
                        let y = extract_i32(tree, "y").unwrap_or(0);
                        let gid = extract_u32(tree, "gid").unwrap_or(0);
                        // For sync, we don't know exact respawn time, use a short default
                        state.depleted_trees.insert(
                            (x, y),
                            crate::game::state::DepletedTreeInfo {
                                gid,
                                depleted_at: now,
                                respawn_at: now + 5.0, // Default 5 seconds remaining
                            },
                        );
                    }
                    log::info!("Synced {} depleted trees", state.depleted_trees.len());
                }
            }
        }

        // =====================================================================
        // Mining Messages
        // =====================================================================
        "miningStarted" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                log::info!("Mining started for player {}", player_id);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_mining = true;
                    player.mining_started_at = macroquad::time::get_time();
                    player.play_attack();
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_mining = true;
                    state.mining_started_at = macroquad::time::get_time();
                }
            }
        }
        "miningStopped" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let reason = extract_string(value, "reason").unwrap_or_default();
                log::info!("Mining stopped for player {}: {}", player_id, reason);
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.is_mining = false;
                }
                if state.local_player_id.as_deref() == Some(&player_id) {
                    state.is_mining = false;
                    if reason == "inventory_full" {
                        state.push_system_chat("Your inventory is full!".to_string());
                        state.pending_sfx.push("error".to_string());
                    }
                }
            }
        }
        "miningSwing" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let rock_x = extract_i32(value, "rock_x").unwrap_or(0);
                let rock_y = extract_i32(value, "rock_y").unwrap_or(0);

                // Check if the action is close enough to the local player to hear
                let is_local = state.local_player_id.as_deref() == Some(&player_id);
                let in_audio_range = is_local || {
                    state
                        .local_player_id
                        .as_ref()
                        .and_then(|id| state.players.get(id))
                        .map(|lp| {
                            let dx = (lp.x - rock_x as f32).abs();
                            let dy = (lp.y - rock_y as f32).abs();
                            dx.max(dy) <= SFX_AUDIBLE_RANGE
                        })
                        .unwrap_or(false)
                };

                // Server says player swung - play the swing animation. Swings are
                // server-authoritative (no local prediction), so this echo is the
                // single source of the animation for everyone, including the local
                // player.
                if let Some(player) = state.players.get_mut(&player_id) {
                    player.play_attack();
                }
                if in_audio_range {
                    state
                        .pending_attack_sounds
                        .push(crate::game::state::AttackSoundType::Melee);

                    // Play mining sound effect
                    state.pending_sfx.push("mining".to_string());
                }

                // Add rock shake effect
                state
                    .rock_shake_effects
                    .push(crate::game::state::RockShakeEffect::new(rock_x, rock_y));

                // Spawn rock debris particles (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let rock_height = 30.0;
                    for _ in 0..4 {
                        state
                            .rock_particles
                            .push(crate::game::state::RockParticle::new_at_rock(
                                rock_x,
                                rock_y,
                                rock_height,
                            ));
                    }
                }
            }
        }
        "miningResult" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                log::info!("Mining result: player {} got {}", player_id, item_id);

                // XP display is handled by the separate skillXp message
                // This handler just shows the item feedback

                if state.local_player_id.as_deref() == Some(player_id.as_str()) {
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|d| d.display_name.clone())
                        .unwrap_or(item_id.clone());

                    // Add chat message about the mine
                    state.push_system_chat(format!("You mined some {}!", item_name));
                }
            }
        }
        "rockDepleted" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let gid = extract_u32(value, "gid").unwrap_or(0);
                let respawn_delay_ms = extract_u64(value, "respawn_delay_ms").unwrap_or(7500);
                let now = macroquad::time::get_time();
                log::info!(
                    "Rock depleted at ({}, {}), respawn in {}ms",
                    x,
                    y,
                    respawn_delay_ms
                );

                // Add crumbling rock effect
                state
                    .crumbling_rocks
                    .push(crate::game::state::CrumblingRockEffect::new(x, y, gid));

                // Spawn a burst of rock debris when rock crumbles (skip on low graphics)
                if !state.ui_state.graphics_low {
                    let rock_height = 30.0;
                    for _ in 0..12 {
                        state
                            .rock_particles
                            .push(crate::game::state::RockParticle::new_at_rock(
                                x,
                                y,
                                rock_height,
                            ));
                    }
                }

                // Mark rock as depleted (hides the static rock, shows respawn timer)
                state.depleted_rocks.insert(
                    (x, y),
                    crate::game::state::DepletedRockInfo {
                        gid,
                        depleted_at: now,
                        respawn_at: now + (respawn_delay_ms as f64 / 1000.0),
                    },
                );
            }
        }
        "rockRespawned" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                log::info!("Rock respawned at ({}, {})", x, y);
                state.depleted_rocks.remove(&(x, y));
            }
        }
        "depletedRocksSync" => {
            if let Some(value) = data {
                if let Some(rocks_arr) = extract_array(value, "rocks") {
                    state.depleted_rocks.clear();
                    let now = macroquad::time::get_time();
                    for rock in rocks_arr {
                        let x = extract_i32(rock, "x").unwrap_or(0);
                        let y = extract_i32(rock, "y").unwrap_or(0);
                        let gid = extract_u32(rock, "gid").unwrap_or(0);
                        // For sync, we don't know exact respawn time, use a short default
                        state.depleted_rocks.insert(
                            (x, y),
                            crate::game::state::DepletedRockInfo {
                                gid,
                                depleted_at: now,
                                respawn_at: now + 5.0, // Default 5 seconds remaining
                            },
                        );
                    }
                    log::info!("Synced {} depleted rocks", state.depleted_rocks.len());
                }
            }
        }
        "farmingPatchStates" => {
            if let Some(value) = data {
                if let Some(patches_arr) = extract_array(value, "patches") {
                    state.farming_patches.clear();
                    state.farming_patch_positions.clear();
                    for p in patches_arr {
                        let patch_id = extract_string(p, "patch_id").unwrap_or_default();
                        let x = extract_i32(p, "x").unwrap_or(0);
                        let y = extract_i32(p, "y").unwrap_or(0);
                        let patch_state =
                            extract_string(p, "state").unwrap_or_else(|| "empty".to_string());
                        let crop_id = extract_string(p, "crop_id").unwrap_or_default();
                        let growth_stage = extract_u32(p, "growth_stage").unwrap_or(0);
                        let owner_id = extract_string(p, "owner_id").unwrap_or_default();
                        state
                            .farming_patch_positions
                            .insert((x, y), patch_id.clone());
                        state.farming_patches.insert(
                            patch_id.clone(),
                            FarmingPatch {
                                patch_id,
                                x,
                                y,
                                state: patch_state,
                                crop_id,
                                growth_stage,
                                owner_id,
                            },
                        );
                    }
                    // Parse unlocked plots
                    if let Some(plots_arr) = extract_array(value, "unlocked_plots") {
                        state.unlocked_farming_plots = plots_arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|u| u as u32))
                            .collect();
                    } else {
                        state.unlocked_farming_plots = vec![1];
                    }

                    // Parse ground tile overrides (farming plot tiles)
                    state.ground_tile_overrides.clear();
                    if let Some(overrides_arr) = extract_array(value, "tile_overrides") {
                        for t in overrides_arr {
                            let x = extract_i32(t, "x").unwrap_or(0);
                            let y = extract_i32(t, "y").unwrap_or(0);
                            let tile_id = extract_u32(t, "tile_id").unwrap_or(0);
                            state.ground_tile_overrides.insert((x, y), tile_id);
                        }
                    }

                    log::info!(
                        "Received {} farming patches, {} tile overrides",
                        state.farming_patches.len(),
                        state.ground_tile_overrides.len()
                    );
                }
            }
        }
        "patchStateUpdate" => {
            if let Some(value) = data {
                let patch_id = extract_string(value, "patch_id").unwrap_or_default();
                let patch_state =
                    extract_string(value, "state").unwrap_or_else(|| "empty".to_string());
                let crop_id = extract_string(value, "crop_id").unwrap_or_default();
                let growth_stage = extract_u32(value, "growth_stage").unwrap_or(0);
                let owner_id = extract_string(value, "owner_id").unwrap_or_default();

                if let Some(patch) = state.farming_patches.get_mut(&patch_id) {
                    // Detect harvest: was harvestable, now empty
                    if patch.state == "harvestable" && patch_state == "empty" {
                        state.pending_sfx.push("pop".to_string());
                    }
                    patch.state = patch_state;
                    patch.crop_id = crop_id;
                    patch.growth_stage = growth_stage;
                    patch.owner_id = owner_id;
                }
            }
        }
        "resourceContractUpdate" => {
            if let Some(value) = data {
                let active = extract_bool(value, "active").unwrap_or(false);
                if active {
                    state.resource_contract = Some(crate::game::ResourceContractInfo {
                        contract_kind: extract_string(value, "contract_kind").unwrap_or_default(),
                        difficulty: extract_string(value, "difficulty").unwrap_or_default(),
                        task_text: extract_string(value, "task_text").unwrap_or_default(),
                        progress_label: extract_string(value, "progress_label").unwrap_or_default(),
                        target_item_id: extract_string(value, "target_item_id").unwrap_or_default(),
                        amount_required: extract_i32(value, "amount_required").unwrap_or(0),
                        amount_completed: extract_i32(value, "amount_completed").unwrap_or(0),
                        giver_name: extract_string(value, "giver_name").unwrap_or_default(),
                    });
                } else {
                    state.resource_contract = None;
                }
            }
        }

        // =====================================================================
        // Friend System Messages
        // =====================================================================
        _ => return false,
    }
    true
}
