use super::*;

#[derive(Deserialize)]
pub(super) struct WsQuery {
    /// Signed session token
    #[serde(rename = "sessionToken")]
    session_token: String,
}

// ============================================================================
// Authenticated WebSocket Handler
// ============================================================================

pub(super) async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate signed session token
    let session_id = match state.token_signer.validate_token(&query.session_token) {
        Some((sid, rid)) => {
            if rid != room_id {
                warn!(
                    "WebSocket rejected: Token room_id mismatch ({} != {})",
                    rid, room_id
                );
                return (
                    StatusCode::FORBIDDEN,
                    "Invalid session token: room mismatch",
                )
                    .into_response();
            }
            sid
        }
        None => {
            warn!("WebSocket rejected: Invalid or expired session token");
            return (StatusCode::UNAUTHORIZED, "Invalid or expired session token").into_response();
        }
    };

    // Validate session exists in our store
    let session_data = state.sessions.get(&session_id).map(|s| s.clone());

    match session_data {
        Some(session) if session.room_id == room_id => {
            // Verify the auth token is still valid
            if !state.auth_sessions.contains_key(&session.auth_token) {
                warn!(
                    "WebSocket rejected: Auth token expired for session {}",
                    session_id
                );
                return (
                    StatusCode::UNAUTHORIZED,
                    "Auth token expired. Please login again.",
                )
                    .into_response();
            }

            // Valid session, upgrade to WebSocket
            let player_id = session.player_id.clone();
            let character_name = session.character_name.clone();
            let character_id = session.character_id;
            let current_map = session.current_map.clone();
            let entrance_x = session.entrance_x;
            let entrance_y = session.entrance_y;
            let is_new_character = session.is_new_character;
            ws.on_upgrade(move |socket| {
                handle_socket(
                    socket,
                    state,
                    room_id,
                    player_id,
                    session_id,
                    character_name,
                    character_id,
                    current_map,
                    entrance_x,
                    entrance_y,
                    is_new_character,
                )
            })
        }
        _ => {
            warn!("Invalid session: {} for room {}", session_id, room_id);
            (StatusCode::FORBIDDEN, "Invalid session").into_response()
        }
    }
}

pub(super) async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    room_id: String,
    player_id: String,
    session_id: String,
    character_name: String,
    character_id: i64,
    current_map: Option<String>, // Interior map to auto-enter on reconnect
    entrance_x: Option<f32>,     // Overworld entrance X (for interior exit)
    entrance_y: Option<f32>,     // Overworld entrance Y (for interior exit)
    is_new_character: bool,      // True if played_time == 0 (for tutorial)
) {
    let (mut sender, mut receiver) = socket.split();

    // Get the room
    let room = match state.rooms.get(&room_id) {
        Some(r) => r.clone(),
        None => {
            error!("Room not found: {}", room_id);
            return;
        }
    };

    // Mark character as online now that WebSocket is actually connected
    state.online_characters.insert(character_id);

    // Activate the player
    let player_name = room.activate_player(&player_id).await;
    info!(
        "Player {} ({}) connected to room {}",
        player_name, player_id, room_id
    );

    // Subscribe to room broadcasts
    let mut broadcast_rx = room.subscribe();

    // Send welcome message
    let welcome = ServerMessage::Welcome {
        player_id: player_id.clone(),
        is_new_character,
    };
    if let Ok(bytes) = protocol::encode_server_message(&welcome) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send entity definitions
    let entity_defs = room.get_entity_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&entity_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send item definitions
    let item_defs = state.item_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&item_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send recipe definitions
    let recipe_defs = state.crafting_registry.to_client_definitions();
    if let Ok(bytes) = protocol::encode_server_message(&recipe_defs) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send discovered recipes
    let discovered = room.get_player_discovered_recipes(&player_id).await;
    let discovered_msg = ServerMessage::DiscoveredRecipes {
        recipes: discovered.into_iter().collect(),
    };
    if let Ok(bytes) = protocol::encode_server_message(&discovered_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send scroll spell definitions
    let scroll_spell_defs_msg = room.get_scroll_spell_definitions_message();
    if let Ok(bytes) = protocol::encode_server_message(&scroll_spell_defs_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send unlocked spells
    let unlocked = room.get_player_unlocked_spells(&player_id).await;
    let unlocked_msg = ServerMessage::UnlockedSpellsSync {
        spell_ids: unlocked.into_iter().collect(),
    };
    if let Ok(bytes) = protocol::encode_server_message(&unlocked_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send gathering marker positions
    let gathering_markers = room.get_gathering_markers_message(None).await;
    if let Ok(bytes) = protocol::encode_server_message(&gathering_markers) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    let world_map = room.get_world_map_message().await;
    if let Ok(bytes) = protocol::encode_server_message(&world_map) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send farming patch states (per-player instanced)
    let farming_patches = room.get_farming_patches_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&farming_patches) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send resource contract state
    let contract_msg = room.get_resource_contract_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&contract_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send crafting order tracker state (if player has an active order)
    if let Some(order_msg) = room.get_crafting_order_tracker_message(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&order_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send chair positions
    let chair_positions = room.get_chair_positions_message().await;
    if let Ok(bytes) = protocol::encode_server_message(&chair_positions) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send overworld chest positions
    let chest_positions = room.get_chest_positions_message(None).await;
    if let Ok(bytes) = protocol::encode_server_message(&chest_positions) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send prayer state
    if let Some(prayer_state) = room.get_player_prayer_state(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&prayer_state) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send collection log definitions
    let clog_defs_msg = protocol::ServerMessage::CollectionLogDefinitions {
        entries: state.collection_log_defs.all_entries(),
        display_names: state.collection_log_display_names.as_ref().clone(),
    };
    if let Ok(bytes) = protocol::encode_server_message(&clog_defs_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send collection log sync (player's obtained entries)
    match state.db.load_collection_log(character_id).await {
        Ok(entries) => {
            let clog_sync = protocol::ServerMessage::CollectionLogSync { entries };
            if let Ok(bytes) = protocol::encode_server_message(&clog_sync) {
                let _ = sender.send(Message::Binary(bytes)).await;
            }
        }
        Err(e) => {
            tracing::warn!("Failed to load collection log for sync: {}", e);
        }
    }

    // Only send overworld data if the player is NOT reconnecting into an instance
    let reconnecting_to_instance = current_map.is_some();

    if !reconnecting_to_instance {
        // Get player's position and send nearby chunks
        if let Some((px, py)) = room.get_player_position(&player_id).await {
            let player_chunk = chunk::ChunkCoord::from_world(px, py);

            // Preload and send chunks in a 3x3 area around the player
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let coord = chunk::ChunkCoord::new(player_chunk.x + dx, player_chunk.y + dy);
                    if let Some(chunk_msg) = room.handle_chunk_request(coord.x, coord.y).await {
                        if let Ok(bytes) = protocol::encode_server_message(&chunk_msg) {
                            let _ = sender.send(Message::Binary(bytes)).await;
                        }
                    }
                }
            }
        }

        // Send existing players inside this client's visibility area.
        {
            for existing_player in room.get_visible_players(&player_id).await {
                let msg = ServerMessage::PlayerJoined {
                    id: existing_player.id.clone(),
                    name: existing_player.name.clone(),
                    x: existing_player.x,
                    y: existing_player.y,
                    gender: existing_player.gender.clone(),
                    skin: existing_player.skin.clone(),
                    hair_style: existing_player.hair_style,
                    hair_color: existing_player.hair_color,
                };
                if let Ok(bytes) = protocol::encode_server_message(&msg) {
                    let _ = sender.send(Message::Binary(bytes)).await;
                }
            }
        }

        // Send existing overworld ground items to this client
        let ground_items = room.get_visible_ground_items(&player_id).await;
        for item_msg in ground_items {
            if let Ok(bytes) = protocol::encode_server_message(&item_msg) {
                let _ = sender.send(Message::Binary(bytes)).await;
            }
        }
    }

    // Notify others about this player joining
    // Instance players will ignore this via state sync filtering
    let (x, y) = room.get_player_position(&player_id).await.unwrap_or((0, 0));
    let (gender, skin) = room
        .get_player_appearance(&player_id)
        .await
        .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
    let (hair_style, hair_color) = room
        .get_player_hair(&player_id)
        .await
        .unwrap_or((None, None));
    let player_joined_msg = ServerMessage::PlayerJoined {
        id: player_id.clone(),
        name: player_name.clone(),
        x,
        y,
        gender: gender.clone(),
        skin: skin.clone(),
        hair_style,
        hair_color,
    };

    // Send PlayerJoined directly to this client first (so player exists before skills sync)
    if let Ok(bytes) = protocol::encode_server_message(&player_joined_msg) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Notify other overworld players (exclude self to avoid double-receive which overwrites skills)
    room.send_to_overworld_players(player_joined_msg, Some(&player_id))
        .await;

    // If player was sitting on a chair, send SitResult so client shows sitting animation
    if let Some((sx, sy, direction)) = room.get_player_sitting_info(&player_id).await {
        let sit_msg = ServerMessage::SitResult {
            success: true,
            tile_x: sx,
            tile_y: sy,
            direction,
        };
        if let Ok(bytes) = protocol::encode_server_message(&sit_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Bring skill/gold milestone quest objectives up to date before sending quest state
    room.process_quest_progression_snapshot(&player_id).await;

    // Send active quests to this client (from saved state)
    for quest_msg in room.get_active_quest_messages(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&quest_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send completed quest ids so client can show correct tier lock/completion states after relog
    let quest_state_sync = room.get_completed_quest_sync_message(&player_id).await;
    if let Ok(bytes) = protocol::encode_server_message(&quest_state_sync) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send full quest catalog for the quest panel
    let quest_catalog = room.build_quest_catalog().await;
    if let Ok(bytes) = protocol::encode_server_message(&quest_catalog) {
        let _ = sender.send(Message::Binary(bytes)).await;
    }

    // Send initial inventory to this client
    if let Some(inv_msg) = room.get_player_inventory_update(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&inv_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send initial skills to this client
    if let Some(skills_msg) = room.get_player_skills_sync(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&skills_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send active potion buffs
    if let Some(buffs_msg) = room.get_player_potion_buffs_sync(&player_id).await {
        if let Ok(bytes) = protocol::encode_server_message(&buffs_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send current top total level player (for trophy icon) — refresh from DB and broadcast to all
    room.init_top_level_player().await;
    {
        let top_msg = room.get_top_player_message().await;
        room.broadcast(top_msg.clone()).await;
        if let Ok(bytes) = protocol::encode_server_message(&top_msg) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Send slayer state sync to this client
    {
        let slayer_state = room.get_player_slayer_state(&player_id).await;
        let slayer_task_data =
            slayer_state
                .current_task
                .as_ref()
                .map(|t| crate::protocol::SlayerTaskData {
                    monster_id: t.monster_id.clone(),
                    display_name: t.display_name.clone(),
                    kills_current: t.kills_current,
                    kills_required: t.kills_required,
                    xp_per_kill: t.xp_per_kill,
                    master_id: t.master_id.clone(),
                    points_on_complete: t.points_on_complete,
                });
        let slayer_sync = ServerMessage::SlayerStateSync {
            current_task: slayer_task_data,
            points: slayer_state.points,
            tasks_completed: slayer_state.tasks_completed,
            blocked_monsters: slayer_state.blocked_monsters.clone(),
            unlocked_monsters: slayer_state.unlocked_monsters.clone(),
        };
        if let Ok(bytes) = protocol::encode_server_message(&slayer_sync) {
            let _ = sender.send(Message::Binary(bytes)).await;
        }
    }

    // Create channel for sending messages to this client
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(512);

    // SECURITY: Register this player's sender for unicast messages
    room.register_player_sender(&player_id, tx).await;

    // Send friends list and pending requests (must be after sender is registered)
    room.send_friends_data(&player_id, &state.online_characters)
        .await;

    // Notify friends that this player came online
    room.broadcast_friend_status(&player_id, true).await;

    // If player was in an instance when they disconnected, auto-re-enter it
    if let Some(ref map_id) = current_map {
        info!(
            "Auto-re-entering instance '{}' for reconnecting player {}",
            map_id, player_id
        );
        auto_enter_instance(&state, &room, &player_id, map_id, entrance_x, entrance_y).await;
    }

    // Spawn task to forward messages to WebSocket
    let send_player_id = player_id.clone();
    let send_perf = state.perf_metrics.clone();
    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
        ping_interval.tick().await; // consume immediate first tick
        loop {
            tokio::select! {
                // Bias toward unicast (StateSync) over broadcasts to prevent
                // broadcast floods from starving position updates
                biased;

                // Handle direct messages to this client (StateSync, etc.)
                Some(msg) = rx.recv() => {
                    let send_start = std::time::Instant::now();
                    let msg_len = msg.len();
                    if sender.send(Message::Binary(msg)).await.is_err() {
                        break;
                    }
                    let send_ms = send_start.elapsed().as_secs_f64() * 1000.0;
                    send_perf.record_ws_send("unicast", send_ms, msg_len);
                    if send_ms > 50.0 {
                        tracing::warn!("Slow WS send (unicast): {:.2}ms, {}B for {}", send_ms, msg_len, send_player_id);
                    }
                }
                // Handle broadcast messages (pre-encoded bytes)
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(bytes) => {
                            let send_start = std::time::Instant::now();
                            let msg_len = bytes.len();
                            if sender.send(Message::Binary(bytes)).await.is_err() {
                                break;
                            }
                            let send_ms = send_start.elapsed().as_secs_f64() * 1000.0;
                            send_perf.record_ws_send("broadcast", send_ms, msg_len);
                            if send_ms > 50.0 {
                                tracing::warn!("Slow WS send (broadcast): {:.2}ms, {}B for {}", send_ms, msg_len, send_player_id);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Broadcast lagged for {}: skipped {} messages", send_player_id, n);
                            // Continue - receiver position was auto-advanced
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                // Server-side WebSocket ping to keep connection alive (browsers auto-pong)
                _ = ping_interval.tick() => {
                    if sender.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Handle incoming messages
    let room_clone = room.clone();
    let player_id_clone = player_id.clone();
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        let mut last_app_msg = std::time::Instant::now();
        loop {
            match tokio::time::timeout(Duration::from_secs(15), receiver.next()).await {
                Ok(Some(Ok(msg))) => match msg {
                    Message::Binary(data) => {
                        last_app_msg = std::time::Instant::now();
                        if let Err(e) = handle_client_message(
                            &state_clone,
                            &room_clone,
                            &session_id_clone,
                            &player_id_clone,
                            &data,
                        )
                        .await
                        {
                            match e {
                                ClientMessageError::SessionSuperseded => {
                                    warn!(
                                        "Closing superseded session {} for player {}",
                                        session_id_clone, player_id_clone
                                    );
                                    break;
                                }
                                _ => warn!("Error handling message: {}", e),
                            }
                        }
                    }
                    Message::Close(_) => break,
                    Message::Pong(_) => {
                        // Browser auto-pong keeps connection alive even when tab is backgrounded
                        last_app_msg = std::time::Instant::now();
                    }
                    _ => {
                        if last_app_msg.elapsed() > Duration::from_secs(45) {
                            warn!(
                                "Player {} timed out (no app messages for 45s)",
                                player_id_clone
                            );
                            break;
                        }
                    }
                },
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    // Short timeout expired, check app-level activity
                    if last_app_msg.elapsed() > Duration::from_secs(45) {
                        warn!(
                            "Player {} connection timed out (no data for 45s)",
                            player_id_clone
                        );
                        break;
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Cleanup — atomically claim session ownership first.
    // If another handler already took over this session, skip all cleanup.
    let removed_session = state.sessions.remove(&session_id);
    if removed_session.is_none() {
        info!(
            "Session {} for {} was superseded by takeover, skipping cleanup",
            session_id, character_name
        );
        return;
    }
    let (_, removed_sess) = removed_session.unwrap();
    let character_id = removed_sess.character_id;
    let should_save = state.auth_sessions.contains_key(&removed_sess.auth_token);

    info!(
        "Character {} disconnected from room {}",
        character_name, room_id
    );

    if should_save {
        // Compute played time delta from anchor
        let played_time_delta = state
            .play_time_anchors
            .remove(&character_id)
            .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
            .unwrap_or(0);

        // Save character state to database
        if let Some(mut save_data) = room.get_player_save_data(&player_id).await {
            // Populate entrance position from runtime HashMap
            if save_data.current_map.is_some() {
                let entrance_positions = state.player_entrance_positions.read().await;
                if let Some(&(ex, ey)) = entrance_positions.get(&player_id) {
                    save_data.entrance_x = Some(ex as f32);
                    save_data.entrance_y = Some(ey as f32);
                }
            }
            if let Err(e) = state
                .db
                .save_character(
                    character_id,
                    save_data.x,
                    save_data.y,
                    save_data.z,
                    save_data.hp,
                    save_data.prayer_points,
                    save_data.mp,
                    &save_data.skills,
                    save_data.gold,
                    &save_data.inventory_json,
                    save_data.equipped_head.as_deref(),
                    save_data.equipped_body.as_deref(),
                    save_data.equipped_weapon.as_deref(),
                    save_data.equipped_back.as_deref(),
                    save_data.equipped_feet.as_deref(),
                    save_data.equipped_ring.as_deref(),
                    save_data.equipped_gloves.as_deref(),
                    save_data.equipped_necklace.as_deref(),
                    save_data.equipped_belt.as_deref(),
                    played_time_delta,
                    save_data.current_map.as_deref(),
                    save_data.sitting_at_x,
                    save_data.sitting_at_y,
                    save_data.entrance_x,
                    save_data.entrance_y,
                    &save_data.bank_json,
                    save_data.bank_gold,
                    save_data.bank_max_slots,
                    &save_data.combat_style_prefs,
                )
                .await
            {
                error!(
                    "Failed to save character {} on disconnect: {}",
                    character_name, e
                );
            } else {
                info!(
                    "Saved character {} to database on disconnect (played_time +{}s)",
                    character_name, played_time_delta
                );
            }
        }

        // Save quest state to database
        if character_id > 0 {
            if let Some(quest_state) = room.get_player_quest_state(&player_id).await {
                if let Err(e) = state
                    .db
                    .save_character_quest_state(character_id, &quest_state)
                    .await
                {
                    error!(
                        "Failed to save quest state for {} on disconnect: {}",
                        character_name, e
                    );
                } else if !quest_state.active_quests.is_empty()
                    || !quest_state.completed_quests.is_empty()
                {
                    info!(
                        "Saved quest state for {}: {} active, {} completed",
                        character_name,
                        quest_state.active_quests.len(),
                        quest_state.completed_quests.len()
                    );
                }
            }
        }

        // Save discovered recipes to database
        if character_id > 0 {
            let discovered = room.get_player_discovered_recipes(&player_id).await;
            if let Err(e) = state
                .db
                .save_discovered_recipes(character_id, &discovered)
                .await
            {
                error!(
                    "Failed to save discovered recipes for {}: {}",
                    character_name, e
                );
            }
            if !discovered.is_empty() {
                info!(
                    "Saved {} discovered recipes for {}",
                    discovered.len(),
                    character_name
                );
            }
        }

        // Save unlocked spells to database
        if character_id > 0 {
            let unlocked = room.get_player_unlocked_spells(&player_id).await;
            if let Err(e) = state.db.save_unlocked_spells(character_id, &unlocked).await {
                error!(
                    "Failed to save unlocked spells for {}: {}",
                    character_name, e
                );
            }
        }

        // Save slayer state to database
        if character_id > 0 {
            let slayer_state = room.get_player_slayer_state(&player_id).await;
            if slayer_state.current_task.is_some()
                || slayer_state.tasks_completed > 0
                || slayer_state.points > 0
            {
                if let Err(e) = state
                    .db
                    .save_character_slayer_state(character_id, &slayer_state)
                    .await
                {
                    error!(
                        "Failed to save slayer state for {} on disconnect: {}",
                        character_name, e
                    );
                } else {
                    info!(
                        "Saved slayer state for {}: {} tasks completed, {} points",
                        character_name, slayer_state.tasks_completed, slayer_state.points
                    );
                }
            }
        }
    } else {
        warn!(
            "Skipping save for {} on disconnect: invalid auth",
            character_name
        );
    }

    // Clean up instance tracking when player disconnects
    // IMPORTANT: We must notify instance peers BEFORE unregistering the sender,
    // and use the instance_id directly (not find_player_instance which scans Instance.players
    // and could race with other operations).
    {
        use crate::interior::InstanceType;

        let removed_instance_id = state.player_instances.write().await.remove(&player_id);
        room.reset_sync_state(&player_id).await;
        if let Some(instance_id) = removed_instance_id {
            // Use get_by_instance_id (direct lookup) instead of find_player_instance (scan)
            if let Some(instance) = state.instance_manager.get_by_instance_id(&instance_id) {
                // Get other players BEFORE removing, so we can notify them
                let other_players: Vec<String> = instance
                    .get_player_ids()
                    .await
                    .into_iter()
                    .filter(|id| id != &player_id)
                    .collect();

                let remaining = instance.remove_player(&player_id).await;

                // Notify instance peers that this player left
                for other_id in &other_players {
                    room.send_to_player(
                        other_id,
                        ServerMessage::PlayerLeft {
                            id: player_id.to_string(),
                        },
                    )
                    .await;
                }

                if remaining == 0 && instance.instance_type == InstanceType::Private {
                    if let Some(owner_id) = &instance.owner_id {
                        state
                            .instance_manager
                            .remove_private(owner_id, &instance.map_id);
                    }
                }
            }
        }
    }

    // Clean up entrance position tracking
    state
        .player_entrance_positions
        .write()
        .await
        .remove(&player_id);

    // SECURITY: Unregister player sender before cleanup
    room.unregister_player_sender(&player_id).await;

    // Notify friends that this player went offline
    room.broadcast_friend_status(&player_id, false).await;

    // Mark character as offline
    state.online_characters.remove(&character_id);

    room.remove_player(&player_id).await;

    // Notify overworld players that this player left.
    // If they were in an instance, instance peers were already notified above.
    // Overworld players still need this in case a stale sprite lingers from
    // a missed enter-instance PlayerLeft.
    room.send_to_overworld_players(
        ServerMessage::PlayerLeft {
            id: player_id.clone(),
        },
        None,
    )
    .await;
}
