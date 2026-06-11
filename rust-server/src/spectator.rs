use super::*;

// ============================================================================
// Spectator WebSocket Handler
// ============================================================================

const MAX_SPECTATORS: usize = 50;

pub(super) async fn spectate_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Get or create the main game room
    let room = state.get_or_create_room("game_room").await;

    // Rate limit spectators
    if room.spectator_count().await >= MAX_SPECTATORS {
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many spectators").into_response();
    }

    ws.on_upgrade(move |socket| handle_spectator(socket, state, room))
        .into_response()
}

pub(super) async fn handle_spectator(socket: WebSocket, state: AppState, room: Arc<GameRoom>) {
    let (mut sender, mut receiver) = socket.split();

    let spectator_id = Uuid::new_v4().to_string();
    info!("Spectator {} connected", spectator_id);

    // Send initial chunks around spawn (5x5 area)
    let spawn_chunk =
        chunk::ChunkCoord::from_world(crate::game::WORLD_SPAWN_X, crate::game::WORLD_SPAWN_Y);
    for dy in -2..=2 {
        for dx in -2..=2 {
            let coord = chunk::ChunkCoord::new(spawn_chunk.x + dx, spawn_chunk.y + dy);
            if let Some(chunk_msg) = room.handle_chunk_request(coord.x, coord.y).await {
                if let Ok(bytes) = protocol::encode_server_message(&chunk_msg) {
                    let _ = sender.send(Message::Binary(bytes)).await;
                }
            }
        }
    }

    // Create channel for sending messages to this spectator
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);

    // Register spectator sender so tick loop can send StateSync
    room.add_spectator(&spectator_id, tx.clone()).await;

    // Subscribe to room broadcasts
    let mut broadcast_rx = room.subscribe();

    // Spawn send loop task (forward mpsc + broadcast to WebSocket)
    let send_spectator_id = spectator_id.clone();
    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
        ping_interval.tick().await; // consume immediate first tick
        loop {
            tokio::select! {
                biased;

                // Handle direct messages (spectator StateSync)
                Some(msg) = rx.recv() => {
                    if sender.send(Message::Binary(msg)).await.is_err() {
                        break;
                    }
                }
                // Handle broadcast messages
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(bytes) => {
                            if sender.send(Message::Binary(bytes)).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Broadcast lagged for spectator {}: skipped {} messages", send_spectator_id, n);
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

    // Spawn recv loop task — ignore all messages except SpectatorUpgrade
    // Returns Some((session_id, player_id, character_name, character_id)) if upgraded
    // Shutdown signal: send_task can notify recv_task to stop so it can run cleanup
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let recv_room = room.clone();
    let recv_spectator_id = spectator_id.clone();
    let recv_state = state.clone();
    let recv_tx = tx.clone();
    let mut recv_task = tokio::spawn(async move {
        // Phase 1: Spectator mode — wait for upgrade request
        loop {
            let msg = match tokio::time::timeout(Duration::from_secs(30), receiver.next()).await {
                Ok(Some(Ok(msg))) => msg,
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {
                    warn!(
                        "Spectator {} connection timed out (no data for 30s)",
                        recv_spectator_id
                    );
                    break;
                }
            };
            match msg {
                Message::Binary(data) => {
                    match protocol::decode_client_message(&data) {
                        Ok(ClientMessage::SpectatorUpgrade { session_token }) => {
                            info!(
                                "Spectator {} upgrade requested, token: {}...",
                                recv_spectator_id,
                                &session_token[..session_token.len().min(8)]
                            );

                            // --- Step 1: Validate session token ---
                            let (session_id, room_id) = match recv_state
                                .token_signer
                                .validate_token(&session_token)
                            {
                                Some((sid, rid)) => (sid, rid),
                                None => {
                                    warn!(
                                        "Spectator {} upgrade rejected: invalid or expired session token",
                                        recv_spectator_id
                                    );
                                    let err_msg = ServerMessage::Error {
                                        code: 401,
                                        message: "Invalid or expired session token".to_string(),
                                    };
                                    if let Ok(bytes) = protocol::encode_server_message(&err_msg) {
                                        let _ = recv_tx.send(bytes).await;
                                    }
                                    continue;
                                }
                            };

                            // --- Step 2: Look up session in state.sessions ---
                            let session =
                                match recv_state.sessions.get(&session_id).map(|s| s.clone()) {
                                    Some(s) if s.room_id == room_id => s,
                                    _ => {
                                        warn!(
                                            "Spectator {} upgrade rejected: invalid session {}",
                                            recv_spectator_id, session_id
                                        );
                                        let err_msg = ServerMessage::Error {
                                            code: 403,
                                            message: "Invalid session".to_string(),
                                        };
                                        if let Ok(bytes) = protocol::encode_server_message(&err_msg)
                                        {
                                            let _ = recv_tx.send(bytes).await;
                                        }
                                        continue;
                                    }
                                };

                            // --- Step 3: Verify auth token is still valid ---
                            if !recv_state.auth_sessions.contains_key(&session.auth_token) {
                                warn!(
                                    "Spectator {} upgrade rejected: auth token expired for session {}",
                                    recv_spectator_id, session_id
                                );
                                let err_msg = ServerMessage::Error {
                                    code: 401,
                                    message: "Auth token expired. Please login again.".to_string(),
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&err_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                                continue;
                            }

                            let player_id = session.player_id.clone();
                            let character_name = session.character_name.clone();
                            let character_id = session.character_id;
                            let current_map = session.current_map.clone();
                            let entrance_x = session.entrance_x;
                            let entrance_y = session.entrance_y;
                            let is_new_character = session.is_new_character;

                            // --- Step 4: Remove spectator registration ---
                            recv_room.remove_spectator(&recv_spectator_id).await;
                            info!(
                                "Spectator {} upgrading to player {} ({})",
                                recv_spectator_id, character_name, player_id
                            );

                            // --- Step 5: Register the existing mpsc sender as the player's sender ---
                            recv_room
                                .register_player_sender(&player_id, recv_tx.clone())
                                .await;

                            // --- Step 6: Mark online and activate the player entity ---
                            recv_state.online_characters.insert(character_id);
                            let player_name = recv_room.activate_player(&player_id).await;

                            // --- Step 7: Send all initial data via the mpsc channel ---

                            // Welcome message
                            let welcome = ServerMessage::Welcome {
                                player_id: player_id.clone(),
                                is_new_character,
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&welcome) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Entity definitions
                            let entity_defs = recv_room.get_entity_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&entity_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Item definitions
                            let item_defs = recv_state.item_registry.to_client_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&item_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Recipe definitions
                            let recipe_defs = recv_state.crafting_registry.to_client_definitions();
                            if let Ok(bytes) = protocol::encode_server_message(&recipe_defs) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Discovered recipes
                            let discovered =
                                recv_room.get_player_discovered_recipes(&player_id).await;
                            let discovered_msg = ServerMessage::DiscoveredRecipes {
                                recipes: discovered.into_iter().collect(),
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&discovered_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Scroll spell definitions
                            let scroll_spell_defs_msg =
                                recv_room.get_scroll_spell_definitions_message();
                            if let Ok(bytes) =
                                protocol::encode_server_message(&scroll_spell_defs_msg)
                            {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Unlocked spells
                            let unlocked = recv_room.get_player_unlocked_spells(&player_id).await;
                            let unlocked_msg = ServerMessage::UnlockedSpellsSync {
                                spell_ids: unlocked.into_iter().collect(),
                            };
                            if let Ok(bytes) = protocol::encode_server_message(&unlocked_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Gathering markers
                            let gathering_markers =
                                recv_room.get_gathering_markers_message(None).await;
                            if let Ok(bytes) = protocol::encode_server_message(&gathering_markers) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            let world_map = recv_room.get_world_map_message().await;
                            if let Ok(bytes) = protocol::encode_server_message(&world_map) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Farming patches (per-player)
                            let farming_patches =
                                recv_room.get_farming_patches_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&farming_patches) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Resource contract
                            let contract_msg =
                                recv_room.get_resource_contract_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&contract_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Chair positions
                            let chair_positions = recv_room.get_chair_positions_message().await;
                            if let Ok(bytes) = protocol::encode_server_message(&chair_positions) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Overworld chest positions
                            let chest_positions = recv_room.get_chest_positions_message(None).await;
                            if let Ok(bytes) = protocol::encode_server_message(&chest_positions) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Prayer state
                            if let Some(prayer_state) =
                                recv_room.get_player_prayer_state(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&prayer_state) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Collection log definitions
                            let clog_defs_msg =
                                crate::protocol::ServerMessage::CollectionLogDefinitions {
                                    entries: recv_state.collection_log_defs.all_entries(),
                                    display_names: recv_state
                                        .collection_log_display_names
                                        .as_ref()
                                        .clone(),
                                };
                            if let Ok(bytes) = protocol::encode_server_message(&clog_defs_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Collection log sync (player's obtained entries)
                            match recv_state.db.load_collection_log(character_id).await {
                                Ok(entries) => {
                                    let clog_sync =
                                        crate::protocol::ServerMessage::CollectionLogSync {
                                            entries,
                                        };
                                    if let Ok(bytes) = protocol::encode_server_message(&clog_sync) {
                                        let _ = recv_tx.send(bytes).await;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to load collection log for sync: {}", e);
                                }
                            }

                            // Only send overworld data if not reconnecting into an instance
                            let reconnecting_to_instance = current_map.is_some();

                            if !reconnecting_to_instance {
                                // Send nearby chunks
                                if let Some((px, py)) =
                                    recv_room.get_player_position(&player_id).await
                                {
                                    let player_chunk = chunk::ChunkCoord::from_world(px, py);
                                    for dy in -1..=1 {
                                        for dx in -1..=1 {
                                            let coord = chunk::ChunkCoord::new(
                                                player_chunk.x + dx,
                                                player_chunk.y + dy,
                                            );
                                            if let Some(chunk_msg) = recv_room
                                                .handle_chunk_request(coord.x, coord.y)
                                                .await
                                            {
                                                if let Ok(bytes) =
                                                    protocol::encode_server_message(&chunk_msg)
                                                {
                                                    let _ = recv_tx.send(bytes).await;
                                                }
                                            }
                                        }
                                    }
                                }

                                // Send only players inside the same visibility area.
                                {
                                    for existing_player in
                                        recv_room.get_visible_players(&player_id).await
                                    {
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
                                            let _ = recv_tx.send(bytes).await;
                                        }
                                    }
                                }

                                // Send existing overworld ground items
                                let ground_items =
                                    recv_room.get_visible_ground_items(&player_id).await;
                                for item_msg in ground_items {
                                    if let Ok(bytes) = protocol::encode_server_message(&item_msg) {
                                        let _ = recv_tx.send(bytes).await;
                                    }
                                }
                            }

                            // Broadcast PlayerJoined to others
                            let (x, y) = recv_room
                                .get_player_position(&player_id)
                                .await
                                .unwrap_or((0, 0));
                            let (gender, skin) = recv_room
                                .get_player_appearance(&player_id)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (hair_style, hair_color) = recv_room
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

                            // Send PlayerJoined to self first
                            if let Ok(bytes) = protocol::encode_server_message(&player_joined_msg) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Notify other overworld players
                            recv_room
                                .send_to_overworld_players(player_joined_msg, Some(&player_id))
                                .await;

                            // If player was sitting, send SitResult
                            if let Some((sx, sy, direction)) =
                                recv_room.get_player_sitting_info(&player_id).await
                            {
                                let sit_msg = ServerMessage::SitResult {
                                    success: true,
                                    tile_x: sx,
                                    tile_y: sy,
                                    direction,
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&sit_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Quest progression snapshot
                            recv_room
                                .process_quest_progression_snapshot(&player_id)
                                .await;

                            // Active quests
                            for quest_msg in recv_room.get_active_quest_messages(&player_id).await {
                                if let Ok(bytes) = protocol::encode_server_message(&quest_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Completed quest sync
                            let quest_state_sync =
                                recv_room.get_completed_quest_sync_message(&player_id).await;
                            if let Ok(bytes) = protocol::encode_server_message(&quest_state_sync) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Quest catalog
                            let quest_catalog = recv_room.build_quest_catalog().await;
                            if let Ok(bytes) = protocol::encode_server_message(&quest_catalog) {
                                let _ = recv_tx.send(bytes).await;
                            }

                            // Inventory
                            if let Some(inv_msg) =
                                recv_room.get_player_inventory_update(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&inv_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Skills
                            if let Some(skills_msg) =
                                recv_room.get_player_skills_sync(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&skills_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Potion buffs
                            if let Some(buffs_msg) =
                                recv_room.get_player_potion_buffs_sync(&player_id).await
                            {
                                if let Ok(bytes) = protocol::encode_server_message(&buffs_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Top total level player (trophy icon) — refresh from DB and broadcast to all
                            recv_room.init_top_level_player().await;
                            {
                                let top_msg = recv_room.get_top_player_message().await;
                                recv_room.broadcast(top_msg.clone()).await;
                                if let Ok(bytes) = protocol::encode_server_message(&top_msg) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Slayer state
                            {
                                let slayer_state =
                                    recv_room.get_player_slayer_state(&player_id).await;
                                let slayer_task_data =
                                    slayer_state.current_task.as_ref().map(|t| {
                                        crate::protocol::SlayerTaskData {
                                            monster_id: t.monster_id.clone(),
                                            display_name: t.display_name.clone(),
                                            kills_current: t.kills_current,
                                            kills_required: t.kills_required,
                                            xp_per_kill: t.xp_per_kill,
                                            master_id: t.master_id.clone(),
                                            points_on_complete: t.points_on_complete,
                                        }
                                    });
                                let slayer_sync = ServerMessage::SlayerStateSync {
                                    current_task: slayer_task_data,
                                    points: slayer_state.points,
                                    tasks_completed: slayer_state.tasks_completed,
                                    blocked_monsters: slayer_state.blocked_monsters.clone(),
                                    unlocked_monsters: slayer_state.unlocked_monsters.clone(),
                                };
                                if let Ok(bytes) = protocol::encode_server_message(&slayer_sync) {
                                    let _ = recv_tx.send(bytes).await;
                                }
                            }

                            // Friends data (must be after sender is registered)
                            recv_room
                                .send_friends_data(&player_id, &recv_state.online_characters)
                                .await;

                            // Notify friends that this player came online
                            recv_room.broadcast_friend_status(&player_id, true).await;

                            // Auto-re-enter instance if applicable
                            if let Some(ref map_id) = current_map {
                                info!(
                                    "Auto-re-entering instance '{}' for reconnecting player {}",
                                    map_id, player_id
                                );
                                auto_enter_instance(
                                    &recv_state,
                                    &recv_room,
                                    &player_id,
                                    map_id,
                                    entrance_x,
                                    entrance_y,
                                )
                                .await;
                            }

                            info!(
                                "Spectator {} fully upgraded to player {} ({})",
                                recv_spectator_id, character_name, player_id
                            );

                            // --- Phase 2: Normal player message handling loop ---
                            let mut last_app_msg = std::time::Instant::now();
                            loop {
                                tokio::select! {
                                    biased;
                                    // Check if send_task died (connection broken)
                                    _ = shutdown_rx.changed() => {
                                        warn!("Send task died for upgraded player {}, proceeding to cleanup", player_id);
                                        break;
                                    }
                                    result = tokio::time::timeout(Duration::from_secs(15), receiver.next()) => {
                                        match result {
                                            Ok(Some(Ok(msg))) => match msg {
                                                Message::Binary(data) => {
                                                    last_app_msg = std::time::Instant::now();
                                                    if let Err(e) = handle_client_message(
                                                        &recv_state,
                                                        &recv_room,
                                                        &session_id,
                                                        &player_id,
                                                        &data,
                                                    )
                                                    .await
                                                    {
                                                        match e {
                                                            ClientMessageError::SessionSuperseded => {
                                                                warn!(
                                                                    "Closing superseded upgraded session {} for player {}",
                                                                    session_id, player_id
                                                                );
                                                                break;
                                                            }
                                                            _ => warn!(
                                                                "Error handling message from upgraded player {}: {}",
                                                                player_id, e
                                                            ),
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
                                                        warn!("Upgraded player {} timed out (no app messages for 45s)", player_id);
                                                        break;
                                                    }
                                                }
                                            },
                                            Ok(Some(Err(_))) | Ok(None) => break,
                                            Err(_) => {
                                                // Short timeout expired, check app-level activity
                                                if last_app_msg.elapsed() > Duration::from_secs(45) {
                                                    warn!("Upgraded player {} connection timed out (no data for 45s)", player_id);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // --- Phase 3: Cleanup (player disconnected) ---
                            // Atomically claim session ownership first.
                            // If another handler already took over, skip cleanup.
                            let removed_session = recv_state.sessions.remove(&session_id);
                            if removed_session.is_none() {
                                info!(
                                    "Session {} for {} was superseded by takeover, skipping cleanup",
                                    session_id, character_name
                                );
                                return true;
                            }
                            let (_, removed_sess) = removed_session.unwrap();
                            let character_id = removed_sess.character_id;
                            let should_save = recv_state
                                .auth_sessions
                                .contains_key(&removed_sess.auth_token);

                            info!(
                                "Upgraded player {} ({}) disconnected",
                                character_name, player_id
                            );

                            if should_save {
                                // If player is in a KOTH instance, move them back to
                                // overworld before saving so they don't respawn inside it
                                {
                                    let instance_id = recv_state
                                        .player_instances
                                        .read()
                                        .await
                                        .get(&player_id)
                                        .cloned();
                                    if let Some(ref inst_id) = instance_id {
                                        if let Some((ex, ey)) =
                                            recv_room.get_koth_entrance(inst_id).await
                                        {
                                            // Reset player to overworld position before save
                                            recv_room
                                                .set_player_position_and_z(&player_id, ex, ey, 0)
                                                .await;
                                            // Remove from instance tracking so save doesn't
                                            // record the KOTH map as current_map
                                            recv_state
                                                .player_instances
                                                .write()
                                                .await
                                                .remove(&player_id);
                                        }
                                    }
                                }

                                // Compute played time delta
                                let played_time_delta = recv_state
                                    .play_time_anchors
                                    .remove(&character_id)
                                    .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
                                    .unwrap_or(0);

                                // Save character state
                                if let Some(mut save_data) =
                                    recv_room.get_player_save_data(&player_id).await
                                {
                                    if save_data.current_map.is_some() {
                                        let entrance_positions =
                                            recv_state.player_entrance_positions.read().await;
                                        if let Some(&(ex, ey)) = entrance_positions.get(&player_id)
                                        {
                                            save_data.entrance_x = Some(ex as f32);
                                            save_data.entrance_y = Some(ey as f32);
                                        }
                                    }
                                    if let Err(e) = recv_state
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

                                // Save quest state
                                if character_id > 0 {
                                    if let Some(quest_state) =
                                        recv_room.get_player_quest_state(&player_id).await
                                    {
                                        if let Err(e) = recv_state
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

                                // Save discovered recipes
                                if character_id > 0 {
                                    let discovered =
                                        recv_room.get_player_discovered_recipes(&player_id).await;
                                    if let Err(e) = recv_state
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

                                // Save unlocked spells
                                if character_id > 0 {
                                    let unlocked =
                                        recv_room.get_player_unlocked_spells(&player_id).await;
                                    if let Err(e) = recv_state
                                        .db
                                        .save_unlocked_spells(character_id, &unlocked)
                                        .await
                                    {
                                        error!(
                                            "Failed to save unlocked spells for {}: {}",
                                            character_name, e
                                        );
                                    }
                                }

                                // Save slayer state
                                if character_id > 0 {
                                    let slayer_state =
                                        recv_room.get_player_slayer_state(&player_id).await;
                                    if slayer_state.current_task.is_some()
                                        || slayer_state.tasks_completed > 0
                                        || slayer_state.points > 0
                                    {
                                        if let Err(e) = recv_state
                                            .db
                                            .save_character_slayer_state(
                                                character_id,
                                                &slayer_state,
                                            )
                                            .await
                                        {
                                            error!(
                                                "Failed to save slayer state for {} on disconnect: {}",
                                                character_name, e
                                            );
                                        } else {
                                            info!(
                                                "Saved slayer state for {}: {} tasks completed, {} points",
                                                character_name,
                                                slayer_state.tasks_completed,
                                                slayer_state.points
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

                            // Clean up instance tracking
                            {
                                use crate::interior::InstanceType;

                                let removed_instance_id =
                                    recv_state.player_instances.write().await.remove(&player_id);
                                recv_room.reset_sync_state(&player_id).await;
                                if let Some(instance_id) = removed_instance_id {
                                    // Clean up KOTH session if player was in one
                                    recv_room.cleanup_koth_session(&instance_id).await;
                                    if let Some(instance) =
                                        recv_state.instance_manager.get_by_instance_id(&instance_id)
                                    {
                                        let other_players: Vec<String> = instance
                                            .get_player_ids()
                                            .await
                                            .into_iter()
                                            .filter(|id| id != &player_id)
                                            .collect();

                                        let remaining = instance.remove_player(&player_id).await;

                                        for other_id in &other_players {
                                            recv_room
                                                .send_to_player(
                                                    other_id,
                                                    ServerMessage::PlayerLeft {
                                                        id: player_id.to_string(),
                                                    },
                                                )
                                                .await;
                                        }

                                        if remaining == 0
                                            && instance.instance_type == InstanceType::Private
                                        {
                                            if let Some(owner_id) = &instance.owner_id {
                                                recv_state
                                                    .instance_manager
                                                    .remove_private(owner_id, &instance.map_id);
                                            }
                                        }
                                    }
                                }
                            }

                            // Clean up entrance position tracking
                            recv_state
                                .player_entrance_positions
                                .write()
                                .await
                                .remove(&player_id);

                            // Unregister player sender
                            recv_room.unregister_player_sender(&player_id).await;

                            // Notify friends that this player went offline
                            recv_room.broadcast_friend_status(&player_id, false).await;

                            // Mark character as offline
                            recv_state.online_characters.remove(&character_id);

                            recv_room.remove_player(&player_id).await;

                            // Notify overworld players that this player left
                            recv_room
                                .send_to_overworld_players(
                                    ServerMessage::PlayerLeft {
                                        id: player_id.clone(),
                                    },
                                    None,
                                )
                                .await;

                            // Return true to indicate upgrade happened (spectator already removed)
                            return true;
                        }
                        Ok(_) => {
                            // Ignore all other messages from spectators
                        }
                        Err(_) => {
                            // Ignore decode errors
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Spectator disconnected without upgrading
        false
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => {
            // Send task died (connection broken) — signal recv_task to stop
            // so it can run Phase 3 cleanup instead of being aborted
            let _ = shutdown_tx.send(true);
            // Wait for recv_task to finish cleanup (with safety timeout)
            match tokio::time::timeout(Duration::from_secs(10), recv_task).await {
                Ok(Ok(true)) => {
                    // recv_task completed and handled player cleanup
                    return;
                }
                _ => {
                    // recv_task didn't complete or wasn't upgraded — clean up spectator
                    room.remove_spectator(&spectator_id).await;
                    info!("Spectator {} disconnected (send task ended)", spectator_id);
                }
            }
        }
        result = &mut recv_task => {
            send_task.abort();
            // If recv_task completed with Ok(true), the player cleanup was already handled
            // inside the task. Only need to remove spectator if NOT upgraded.
            let upgraded = result.unwrap_or(false);
            if !upgraded {
                room.remove_spectator(&spectator_id).await;
                info!("Spectator {} disconnected", spectator_id);
            }
        }
    }
}
