use super::*;

/// Auto-enter an instance on reconnect (when current_map was saved in DB)
pub(super) async fn auto_enter_instance(
    state: &AppState,
    room: &GameRoom,
    player_id: &str,
    map_id: &str,
    entrance_x: Option<f32>,
    entrance_y: Option<f32>,
) {
    use crate::interior::InstanceType;
    use crate::protocol::{ChunkLayerData, ChunkPortalData};
    use base64::Engine;

    let interior = match state.content.interior_registry.get(map_id) {
        Some(i) => i,
        None => {
            warn!(
                "Auto-enter: unknown interior '{}' for player {}, staying in overworld",
                map_id, player_id
            );
            return;
        }
    };

    // Use the default spawn point
    let spawn = match interior.spawn_points.values().next() {
        Some(s) => s.clone(),
        None => {
            warn!("Auto-enter: interior '{}' has no spawn points", map_id);
            return;
        }
    };

    // Get or create instance
    let (instance, is_new) = match interior.instance_type {
        InstanceType::Public => state.instance_manager.get_or_create_public(
            &interior.id,
            interior.size.width,
            interior.size.height,
            interior.pvp_enabled,
        ),
        InstanceType::Private => state.instance_manager.get_or_create_private(
            &interior.id,
            player_id,
            interior.size.width,
            interior.size.height,
            interior.pvp_enabled,
        ),
    };

    if is_new || !*instance.npcs_spawned.read().await {
        // Load collision data for NPC walkability
        if !interior.collision.is_empty()
            && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&interior.collision)
        {
            instance.set_collision(&bytes).await;
        }
        instance
            .spawn_npcs(&interior.entities, &state.content.entity_registry)
            .await;

        // Register gathering markers for this instance
        if !interior.gathering_zones.is_empty() {
            let markers: Vec<crate::gathering::GatheringMarker> = interior
                .gathering_zones
                .iter()
                .map(|gz| crate::gathering::GatheringMarker {
                    x: gz.x,
                    y: gz.y,
                    zone_id: gz.zone_id.clone(),
                })
                .collect();
            room.register_instance_gathering_markers(&instance.id, markers)
                .await;
        }
    }

    // Restore entrance position from DB (for use when exiting the interior)
    if let (Some(ex), Some(ey)) = (entrance_x, entrance_y) {
        let mut entrance_positions = state.player_entrance_positions.write().await;
        entrance_positions.insert(player_id.to_string(), (ex as i32, ey as i32));
    }

    // Track player's instance
    {
        let mut player_instances = state.player_instances.write().await;
        player_instances.insert(player_id.to_string(), instance.id.clone());
    }
    room.reset_sync_state(player_id).await;

    // Notify overworld players that this player has "left"
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.to_string(),
        },
        Some(player_id),
    )
    .await;

    // Get other players already in the instance BEFORE adding
    let other_players_in_instance: Vec<String> = instance.get_player_ids().await;

    instance.add_player(player_id).await;
    // Player position is already correct from DB — don't override with spawn point
    let (player_x, player_y) = room
        .get_player_position(player_id)
        .await
        .unwrap_or((spawn.x as i32, spawn.y as i32));

    // Notify instance players
    if !other_players_in_instance.is_empty() {
        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
        let (gender, skin) = room
            .get_player_appearance(player_id)
            .await
            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
        let (hair_style, hair_color) = room
            .get_player_hair(player_id)
            .await
            .unwrap_or((None, None));

        for other_id in &other_players_in_instance {
            room.send_to_player(
                other_id,
                ServerMessage::PlayerJoined {
                    id: player_id.to_string(),
                    name: player_name.clone(),
                    x: player_x,
                    y: player_y,
                    gender: gender.clone(),
                    skin: skin.clone(),
                    hair_style,
                    hair_color,
                },
            )
            .await;
        }

        for other_id in &other_players_in_instance {
            if let Some(other_name) = room.get_player_name(other_id).await {
                let (other_x, other_y) = room
                    .get_player_position(other_id)
                    .await
                    .unwrap_or((spawn.x as i32, spawn.y as i32));
                let (other_gender, other_skin) = room
                    .get_player_appearance(other_id)
                    .await
                    .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                let (other_hair_style, other_hair_color) =
                    room.get_player_hair(other_id).await.unwrap_or((None, None));

                room.send_to_player(
                    player_id,
                    ServerMessage::PlayerJoined {
                        id: other_id.clone(),
                        name: other_name,
                        x: other_x,
                        y: other_y,
                        gender: other_gender,
                        skin: other_skin,
                        hair_style: other_hair_style,
                        hair_color: other_hair_color,
                    },
                )
                .await;
            }
        }
    }

    // Send transition message
    room.send_to_player(
        player_id,
        ServerMessage::MapTransition {
            map_type: "interior".to_string(),
            map_id: interior.id.clone(),
            spawn_x: player_x as f32,
            spawn_y: player_y as f32,
            instance_id: instance.id.clone(),
        },
    )
    .await;

    // Send interior map data
    let layers = vec![
        ChunkLayerData {
            layer_type: 0,
            tiles: interior.layers.ground.clone(),
        },
        ChunkLayerData {
            layer_type: 1,
            tiles: interior.layers.objects.clone(),
        },
        ChunkLayerData {
            layer_type: 2,
            tiles: interior.layers.overhead.clone(),
        },
    ];

    let collision = if interior.collision.is_empty() {
        vec![]
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(&interior.collision)
            .unwrap_or_default()
    };

    let portals: Vec<ChunkPortalData> = interior
        .portals
        .iter()
        .map(|p| ChunkPortalData {
            id: p.id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            target_map: p.target_map.clone(),
            target_spawn: p.target_spawn.clone().unwrap_or_default(),
        })
        .collect();

    let objects: Vec<protocol::ChunkObjectData> = interior
        .map_objects
        .iter()
        .map(|o| protocol::ChunkObjectData {
            gid: o.gid,
            tile_x: o.x,
            tile_y: o.y,
            width: o.width,
            height: o.height,
        })
        .collect();

    let walls: Vec<protocol::ChunkWallData> = interior
        .walls
        .iter()
        .map(|w| protocol::ChunkWallData {
            gid: w.gid,
            tile_x: w.x,
            tile_y: w.y,
            edge: w.edge.clone(),
        })
        .collect();

    room.send_to_player(
        player_id,
        ServerMessage::InteriorData {
            map_id: interior.id.clone(),
            name: interior.name.clone(),
            instance_id: instance.id.clone(),
            width: interior.size.width,
            height: interior.size.height,
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            layers,
            collision,
            portals,
            objects,
            walls,
            heightmap: interior.heightmap.clone(),
            block_types_down: interior.block_types_down.clone(),
            block_types_right: interior.block_types_right.clone(),
        },
    )
    .await;

    // Send NPC updates
    let npc_updates = instance.get_npc_updates().await;
    if !npc_updates.is_empty() {
        room.send_to_player(
            player_id,
            ServerMessage::StateSync {
                tick: 0,
                players: vec![],
                npcs: npc_updates,
                instance_id: instance.id.clone(),
            },
        )
        .await;
    }

    // Send gathering markers for this instance
    room.send_to_player(
        player_id,
        room.get_gathering_markers_message(Some(&instance.id)).await,
    )
    .await;

    // Send ground items
    let ground_items = room.get_visible_ground_items(player_id).await;
    for item_msg in ground_items {
        room.send_to_player(player_id, item_msg).await;
    }

    info!(
        "Auto-entered player {} into instance {} (map: {})",
        player_id, instance.id, map_id
    );
}

pub(super) async fn handle_enter_portal(
    state: &AppState,
    room: &GameRoom,
    player_id: &str,
    portal_id: &str,
) {
    use crate::interior::InstanceType;
    use crate::protocol::{ChunkLayerData, ChunkPortalData};
    use base64::Engine;

    info!(
        "Player {} attempting to enter portal '{}'",
        player_id, portal_id
    );

    // Check if player is currently in an interior
    let current_instance_id = {
        let instances = state.player_instances.read().await;
        instances.get(player_id).cloned()
    };

    // If player is in an interior, handle exit portal
    if let Some(instance_id) = current_instance_id {
        info!(
            "Player {} is in instance '{}', checking for interior exit portal",
            player_id, instance_id
        );

        // Find the interior this instance belongs to
        let interior_id = instance_id
            .strip_prefix("pub_")
            .or_else(|| instance_id.split('_').nth(1))
            .unwrap_or(&instance_id);

        let interior = match state.content.interior_registry.get(interior_id) {
            Some(i) => i,
            None => {
                error!(
                    "Could not find interior definition for instance '{}'",
                    instance_id
                );
                return;
            }
        };

        // Get player position
        let (player_x, player_y) = match room.get_player_position(player_id).await {
            Some(pos) => pos,
            None => {
                warn!("Player {} not found in room", player_id);
                return;
            }
        };

        // Find the portal in the interior's portal list
        let exit_portal = interior.portals.iter().find(|p| {
            p.id == portal_id
                && player_x >= p.x
                && player_x < p.x + p.width
                && player_y >= p.y
                && player_y < p.y + p.height
        });

        match exit_portal {
            Some(portal) => {
                info!(
                    "Found exit portal '{}' targeting '{}' at ({}, {})",
                    portal.id, portal.target_map, portal.target_x, portal.target_y
                );

                // Compute overworld spawn and update position BEFORE removing
                // from instance tracking, so if the tick loop sees the player as
                // overworld, they're already at the correct spawn position
                // (prevents ghost on portal tile).
                let (spawn_x, spawn_y) = if portal.target_map == "overworld" {
                    let coords = if portal.target_x != 0.0 || portal.target_y != 0.0 {
                        // Portal has explicit exit coordinates - use them
                        info!(
                            "Using portal exit coordinates ({}, {}) for player {}",
                            portal.target_x, portal.target_y, player_id
                        );
                        // Clean up stored entrance since we're not using it
                        let mut entrance_positions = state.player_entrance_positions.write().await;
                        entrance_positions.remove(player_id);
                        (portal.target_x, portal.target_y)
                    } else {
                        // Fall back to stored entrance position
                        let mut entrance_positions = state.player_entrance_positions.write().await;
                        if let Some((x, y)) = entrance_positions.remove(player_id) {
                            info!(
                                "Using stored entrance position ({}, {}) for player {}",
                                x, y, player_id
                            );
                            (x as f32, y as f32)
                        } else {
                            // Default spawn if nothing specified
                            (0.0, 0.0)
                        }
                    };

                    info!(
                        "Player {} exiting to overworld at ({}, {})",
                        player_id, coords.0, coords.1
                    );

                    room.set_player_position(player_id, coords.0 as i32, coords.1 as i32)
                        .await;
                    coords
                } else {
                    (0.0, 0.0)
                };

                // Remove player from both tracking systems and notify others.
                // Use get_by_instance_id (direct lookup by known ID) instead of
                // find_player_instance (scan that races with concurrent removals).
                // Position is already updated (for overworld exits) so the tick loop
                // won't see the player at the old portal position.
                {
                    let mut instances = state.player_instances.write().await;
                    instances.remove(player_id);
                }
                // Clean up KOTH state if exiting a KOTH instance
                room.cleanup_koth_session(&instance_id).await;
                room.reset_sync_state(player_id).await;

                if let Some(instance) = state.instance_manager.get_by_instance_id(&instance_id) {
                    // Get other players in the instance BEFORE removing this player
                    let other_players: Vec<String> = instance
                        .get_player_ids()
                        .await
                        .into_iter()
                        .filter(|id| id != player_id)
                        .collect();

                    let remaining = instance.remove_player(player_id).await;
                    if remaining == 0
                        && instance.instance_type == InstanceType::Private
                        && let Some(owner_id) = &instance.owner_id
                    {
                        state
                            .instance_manager
                            .remove_private(owner_id, &instance.map_id);
                    }

                    // Notify other players in the instance that this player left
                    // AND notify the exiting player that those players "left" their view
                    for other_id in &other_players {
                        // Tell players still in instance that this player left
                        room.send_to_player(
                            other_id,
                            ServerMessage::PlayerLeft {
                                id: player_id.to_string(),
                            },
                        )
                        .await;

                        // Tell the exiting player that the instance players are gone from their view
                        room.send_to_player(
                            player_id,
                            ServerMessage::PlayerLeft {
                                id: other_id.clone(),
                            },
                        )
                        .await;
                    }
                }

                if portal.target_map == "overworld" {
                    // Preload chunks around the overworld spawn before transitioning
                    let spawn_chunk = chunk::ChunkCoord::from_world(
                        spawn_x.floor() as i32,
                        spawn_y.floor() as i32,
                    );
                    room.world()
                        .preload_chunks(spawn_chunk, game::SPAWN_PRELOAD_RADIUS)
                        .await;

                    // Send transition back to overworld
                    room.send_to_player(
                        player_id,
                        ServerMessage::MapTransition {
                            map_type: "overworld".to_string(),
                            map_id: "world_0".to_string(),
                            spawn_x,
                            spawn_y,
                            instance_id: String::new(),
                        },
                    )
                    .await;

                    // Re-send overworld data that was cleared on instance entry
                    room.send_to_player(player_id, room.get_chair_positions_message().await)
                        .await;
                    room.send_to_player(player_id, room.get_gathering_markers_message(None).await)
                        .await;
                    room.send_to_player(player_id, room.get_chest_positions_message(None).await)
                        .await;

                    // Send overworld ground items
                    for item_msg in room.get_visible_ground_items(player_id).await {
                        room.send_to_player(player_id, item_msg).await;
                    }

                    // Notify overworld players that this player has returned
                    {
                        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
                        let (gender, skin) = room
                            .get_player_appearance(player_id)
                            .await
                            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                        let (hair_style, hair_color) = room
                            .get_player_hair(player_id)
                            .await
                            .unwrap_or((None, None));
                        room.send_to_overworld_players(
                            ServerMessage::PlayerJoined {
                                id: player_id.to_string(),
                                name: player_name,
                                x: spawn_x as i32,
                                y: spawn_y as i32,
                                gender,
                                skin,
                                hair_style,
                                hair_color,
                            },
                            Some(player_id),
                        )
                        .await;
                    }

                    return;
                } else {
                    // Portal leads to another interior - fall through to normal handling
                    // (would need to update portal struct to work with interior->interior)
                    info!(
                        "Portal leads to another interior '{}' - not yet supported",
                        portal.target_map
                    );
                    return;
                }
            }
            None => {
                warn!(
                    "Player {} tried to use portal '{}' but no matching exit portal found at ({}, {})",
                    player_id, portal_id, player_x, player_y
                );
                return;
            }
        }
    }

    // Player is in overworld - find portal in world chunks
    let portal = match room.find_portal_at_player(player_id).await {
        Some(p) => {
            info!(
                "Found portal at player position: id='{}', target_map='{}', target_spawn='{}'",
                p.id, p.target_map, p.target_spawn
            );
            if p.id == portal_id {
                p
            } else {
                warn!(
                    "Player {} tried to use portal '{}' but is standing on portal '{}'",
                    player_id, portal_id, p.id
                );
                return;
            }
        }
        None => {
            warn!(
                "Player {} tried to use portal '{}' but no portal found at position",
                player_id, portal_id
            );
            return;
        }
    };

    // Get interior definition
    info!("Looking up interior map '{}'", portal.target_map);
    let interior = match state.content.interior_registry.get(&portal.target_map) {
        Some(i) => {
            info!(
                "Found interior '{}' with {} spawn points",
                i.id,
                i.spawn_points.len()
            );
            i
        }
        None => {
            error!(
                "Portal '{}' references unknown interior '{}'. Available interiors: {:?}",
                portal_id,
                portal.target_map,
                state.content.interior_registry.list_ids()
            );
            return;
        }
    };

    // Check if this interior requires an active slayer task
    if interior.requires_slayer_task {
        let slayer_state = room.get_player_slayer_state(player_id).await;
        if slayer_state.current_task.is_none() {
            room.send_to_player(
                player_id,
                ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: "[System]".to_string(),
                    text: "You need an active slayer task to enter this cave.".to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    channel: "system".to_string(),
                },
            )
            .await;
            return;
        }
    }

    // Get spawn point - try exact name, then "entrance", then first available
    info!(
        "Looking up spawn point '{}' in interior '{}'",
        portal.target_spawn, interior.id
    );
    let spawn = if !portal.target_spawn.is_empty() {
        interior.get_spawn_point(&portal.target_spawn)
    } else {
        None
    };
    let spawn = match spawn
        .or_else(|| interior.get_spawn_point("entrance"))
        .or_else(|| interior.spawn_points.values().next())
    {
        Some(s) => {
            info!(
                "Using spawn point at ({}, {}) in interior '{}'",
                s.x, s.y, interior.id
            );
            s
        }
        None => {
            error!("Interior '{}' has no spawn points at all!", interior.id);
            return;
        }
    };

    // Get or create instance based on type
    let (instance, is_new) = match interior.instance_type {
        InstanceType::Public => state.instance_manager.get_or_create_public(
            &interior.id,
            interior.size.width,
            interior.size.height,
            interior.pvp_enabled,
        ),
        InstanceType::Private => state.instance_manager.get_or_create_private(
            &interior.id,
            player_id,
            interior.size.width,
            interior.size.height,
            interior.pvp_enabled,
        ),
    };

    // Spawn NPCs if this is a new instance
    if is_new || !*instance.npcs_spawned.read().await {
        // Load collision data for NPC walkability
        if !interior.collision.is_empty()
            && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&interior.collision)
        {
            instance.set_collision(&bytes).await;
        }
        // Load heightmap if present
        if let Some(ref hm) = interior.heightmap {
            instance.set_heightmap(hm.clone()).await;
        }
        instance
            .spawn_npcs(&interior.entities, &state.content.entity_registry)
            .await;

        // Register gathering markers for this instance
        if !interior.gathering_zones.is_empty() {
            let markers: Vec<crate::gathering::GatheringMarker> = interior
                .gathering_zones
                .iter()
                .map(|gz| crate::gathering::GatheringMarker {
                    x: gz.x,
                    y: gz.y,
                    zone_id: gz.zone_id.clone(),
                })
                .collect();
            room.register_instance_gathering_markers(&instance.id, markers)
                .await;
        }
    }

    // Store player's entrance position (where they came from) for return teleport
    if let Some((entrance_x, entrance_y)) = room.get_player_position(player_id).await {
        let mut entrance_positions = state.player_entrance_positions.write().await;
        entrance_positions.insert(player_id.to_string(), (entrance_x, entrance_y));
        info!(
            "Stored entrance position ({}, {}) for player {}",
            entrance_x, entrance_y, player_id
        );
    }

    // Track player's instance
    {
        let mut player_instances = state.player_instances.write().await;
        player_instances.insert(player_id.to_string(), instance.id.clone());
    }
    room.reset_sync_state(player_id).await;

    // Notify overworld players that this player has "left" (so they don't see a frozen sprite)
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.to_string(),
        },
        Some(player_id),
    )
    .await;

    // Get other players already in the instance BEFORE adding this player
    let other_players_in_instance: Vec<String> = instance.get_player_ids().await;

    // Add player to instance
    instance.add_player(player_id).await;

    // Update player position to spawn point, including Z from heightmap
    let spawn_z = {
        let hm = instance.heightmap.read().await;
        instance.get_height_at_sync(&hm, spawn.x as i32, spawn.y as i32)
    };
    room.set_player_position_and_z(player_id, spawn.x as i32, spawn.y as i32, spawn_z)
        .await;

    // Notify other players in the instance that this player joined
    if !other_players_in_instance.is_empty() {
        let player_name = room.get_player_name(player_id).await.unwrap_or_default();
        let (gender, skin) = room
            .get_player_appearance(player_id)
            .await
            .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
        let (hair_style, hair_color) = room
            .get_player_hair(player_id)
            .await
            .unwrap_or((None, None));

        for other_id in &other_players_in_instance {
            room.send_to_player(
                other_id,
                ServerMessage::PlayerJoined {
                    id: player_id.to_string(),
                    name: player_name.clone(),
                    x: spawn.x as i32,
                    y: spawn.y as i32,
                    gender: gender.clone(),
                    skin: skin.clone(),
                    hair_style,
                    hair_color,
                },
            )
            .await;
        }

        // Also send existing instance players to the joining player
        for other_id in &other_players_in_instance {
            if let Some(other_name) = room.get_player_name(other_id).await {
                let (other_x, other_y) = room
                    .get_player_position(other_id)
                    .await
                    .unwrap_or((spawn.x as i32, spawn.y as i32));
                let (other_gender, other_skin) = room
                    .get_player_appearance(other_id)
                    .await
                    .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                let (other_hair_style, other_hair_color) =
                    room.get_player_hair(other_id).await.unwrap_or((None, None));

                room.send_to_player(
                    player_id,
                    ServerMessage::PlayerJoined {
                        id: other_id.clone(),
                        name: other_name,
                        x: other_x,
                        y: other_y,
                        gender: other_gender,
                        skin: other_skin,
                        hair_style: other_hair_style,
                        hair_color: other_hair_color,
                    },
                )
                .await;
            }
        }
    }

    info!(
        "Player {} entered instance {} (map: {}) at ({}, {})",
        player_id, instance.id, interior.id, spawn.x, spawn.y
    );

    // Start KOTH session if entering KOTH arena
    if interior.id == crate::game::koth_tick::KOTH_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // Save the player's overworld position so we can teleport them back
        let (entrance_x, entrance_y) = room.get_player_position(player_id).await.unwrap_or((0, 0));
        room.start_koth_session(
            &instance.id,
            player_id,
            interior.size.width,
            interior.size.height,
            ct,
            entrance_x,
            entrance_y,
        )
        .await;
    }

    // Start or join boss session if entering desert wurm arena
    if interior.id == crate::game::boss_tick::BOSS_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if room.has_boss_session(&instance.id).await {
            // Add player to existing boss fight
            room.add_boss_player(&instance.id, player_id).await;
        } else {
            // Find the desert_wurm NPC in the instance and start a new boss session
            let npcs = instance.npcs.read().await;
            if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "desert_wurm") {
                room.start_boss_session(
                    &instance.id,
                    &boss_npc.id,
                    boss_npc.hp,
                    boss_npc.max_hp,
                    boss_npc.x,
                    boss_npc.y,
                    instance.map_width as i32,
                    instance.map_height as i32,
                    ct,
                )
                .await;
            }
        }
    }

    // Start or join pharaoh boss session if entering pyramid tomb
    if interior.id == crate::game::boss_tick::PHARAOH_BOSS_MAP_ID {
        let ct = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if room.has_pharaoh_boss_session(&instance.id).await {
            room.add_pharaoh_boss_player(&instance.id, player_id).await;
        } else {
            let npcs = instance.npcs.read().await;
            if let Some(boss_npc) = npcs.values().find(|n| n.prototype_id == "khareth_pharaoh") {
                room.start_pharaoh_boss_session(
                    &instance.id,
                    &boss_npc.id,
                    boss_npc.hp,
                    boss_npc.max_hp,
                    boss_npc.x,
                    boss_npc.y,
                    instance.map_width as i32,
                    instance.map_height as i32,
                    ct,
                )
                .await;
            }
        }
    }

    // Send transition message to client
    room.send_to_player(
        player_id,
        ServerMessage::MapTransition {
            map_type: "interior".to_string(),
            map_id: interior.id.clone(),
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            instance_id: instance.id.clone(),
        },
    )
    .await;

    // Send interior map data
    let layers = vec![
        ChunkLayerData {
            layer_type: 0,
            tiles: interior.layers.ground.clone(),
        },
        ChunkLayerData {
            layer_type: 1,
            tiles: interior.layers.objects.clone(),
        },
        ChunkLayerData {
            layer_type: 2,
            tiles: interior.layers.overhead.clone(),
        },
    ];

    let collision = if interior.collision.is_empty() {
        vec![]
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(&interior.collision)
            .unwrap_or_default()
    };

    let portals: Vec<ChunkPortalData> = interior
        .portals
        .iter()
        .map(|p| ChunkPortalData {
            id: p.id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            target_map: p.target_map.clone(),
            target_spawn: p.target_spawn.clone().unwrap_or_default(),
        })
        .collect();

    let objects: Vec<protocol::ChunkObjectData> = interior
        .map_objects
        .iter()
        .map(|o| protocol::ChunkObjectData {
            gid: o.gid,
            tile_x: o.x,
            tile_y: o.y,
            width: o.width,
            height: o.height,
        })
        .collect();

    let walls: Vec<protocol::ChunkWallData> = interior
        .walls
        .iter()
        .map(|w| protocol::ChunkWallData {
            gid: w.gid,
            tile_x: w.x,
            tile_y: w.y,
            edge: w.edge.clone(),
        })
        .collect();

    room.send_to_player(
        player_id,
        ServerMessage::InteriorData {
            map_id: interior.id.clone(),
            name: interior.name.clone(),
            instance_id: instance.id.clone(),
            width: interior.size.width,
            height: interior.size.height,
            spawn_x: spawn.x,
            spawn_y: spawn.y,
            layers,
            collision,
            portals,
            objects,
            walls,
            heightmap: interior.heightmap.clone(),
            block_types_down: interior.block_types_down.clone(),
            block_types_right: interior.block_types_right.clone(),
        },
    )
    .await;

    // Send NPC updates for this instance
    let npc_updates = instance.get_npc_updates().await;
    if !npc_updates.is_empty() {
        info!(
            "Sending {} instance NPCs to player {}",
            npc_updates.len(),
            player_id
        );
        room.send_to_player(
            player_id,
            ServerMessage::StateSync {
                tick: 0,
                players: vec![],
                npcs: npc_updates,
                instance_id: instance.id.clone(),
            },
        )
        .await;
    }

    // Send gathering markers for this instance
    room.send_to_player(
        player_id,
        room.get_gathering_markers_message(Some(&instance.id)).await,
    )
    .await;

    // Send existing ground items in this instance
    let ground_items = room.get_visible_ground_items(player_id).await;
    for item_msg in ground_items {
        room.send_to_player(player_id, item_msg).await;
    }

    // Send chest positions for this interior
    let chest_msg = room.get_chest_positions_message(Some(&interior.id)).await;
    if let protocol::ServerMessage::ChestPositions { ref positions } = chest_msg {
        info!(
            "Sending {} chest positions for interior '{}' to player {}",
            positions.len(),
            interior.id,
            player_id
        );
    }
    room.send_to_player(player_id, chest_msg).await;
}
