use super::*;

// ============================================================================
// HTTP Handlers - Matchmaking
// ============================================================================

#[derive(Deserialize)]
pub(super) struct JoinOptions {
    #[serde(rename = "characterId")]
    character_id: i64,
}

#[derive(Serialize)]
struct MatchmakeResponse {
    room: RoomInfo,
    /// Signed session token for WebSocket upgrade (expires in 5 minutes)
    #[serde(rename = "sessionToken")]
    session_token: String,
}

#[derive(Serialize)]
struct RoomInfo {
    #[serde(rename = "roomId")]
    room_id: String,
    name: String,
    clients: usize,
}

pub(super) async fn matchmake_join_or_create(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(room_name): Path<String>,
    headers: axum::http::HeaderMap,
    Json(options): Json<JoinOptions>,
) -> impl IntoResponse {
    let client_ip = addr.ip().to_string();

    if room_name != GAME_ROOM_NAME {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Unknown game room" })),
        )
            .into_response();
    }

    if !state.matchmake_rate_limiter.check(&client_ip) {
        warn!("Rate limit exceeded for matchmaking from {}", client_ip);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "Too many requests. Please try again later." })),
        )
            .into_response();
    }

    let auth_token = match headers.get("Authorization") {
        Some(auth_header) => match auth_header.to_str() {
            Ok(auth_str) => match auth_str.strip_prefix("Bearer ") {
                Some(token) => token.to_string(),
                None => {
                    warn!("Matchmaking rejected: Invalid Authorization format");
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({
                            "error": "Invalid authorization format. Use 'Bearer <token>'"
                        })),
                    )
                        .into_response();
                }
            },
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "Invalid authorization header" })),
                )
                    .into_response();
            }
        },
        None => {
            warn!("Matchmaking rejected: No Authorization header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Authorization required. Please login first." })),
            )
                .into_response();
        }
    };

    // Validate token and get authenticated account info
    let (account_id, _username) = match state.auth_sessions.get(&auth_token) {
        Some(auth_data) => auth_data.clone(),
        None => {
            warn!("Matchmaking rejected: Invalid or expired token");
            return (
                StatusCode::UNAUTHORIZED,
                Json(
                    serde_json::json!({ "error": "Invalid or expired token. Please login again." }),
                ),
            )
                .into_response();
        }
    };

    // Check for active ban on this account
    if let Some((reason, expires_at)) = state.db.check_ban_by_account(account_id).await {
        let msg = match reason {
            Some(r) => format!("Account banned until {}. Reason: {}", expires_at, r),
            None => format!("Account banned until {}.", expires_at),
        };
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response();
    }

    // Load the specified character and verify ownership
    let character_id = options.character_id;
    let character_data = match state.db.get_character(character_id).await {
        Ok(Some(char)) => {
            if char.account_id != account_id {
                warn!(
                    "Matchmaking rejected: Character {} does not belong to account {}",
                    character_id, account_id
                );
                return (
                    StatusCode::FORBIDDEN,
                    Json(
                        serde_json::json!({ "error": "Character does not belong to this account" }),
                    ),
                )
                    .into_response();
            }
            char
        }
        Ok(None) => {
            warn!("Matchmaking rejected: Character {} not found", character_id);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Character not found" })),
            )
                .into_response();
        }
        Err(e) => {
            error!("Failed to load character {}: {}", character_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to load character" })),
            )
                .into_response();
        }
    };

    let character_session_lock = state
        .character_session_locks
        .entry(character_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _character_session_guard = character_session_lock.lock().await;

    // Check if character is already online — attempt session takeover
    let has_reserved_session = state
        .sessions
        .iter()
        .any(|session| session.character_id == character_id);
    if state.online_characters.contains(&character_id) || has_reserved_session {
        // Find the old session for this character
        let old_session_id = state
            .sessions
            .iter()
            .find(|entry| entry.value().character_id == character_id)
            .map(|entry| entry.key().clone());

        if let Some(old_sid) = old_session_id {
            // Stop new commands and wait for any in-flight command to finish
            // before saving or replacing the shared player entity.
            if let Some(old_session) = state.sessions.get(&old_sid).map(|entry| entry.clone()) {
                let mut command_gate = old_session.command_gate.write().await;
                *command_gate = false;
            }

            // Atomically remove old session — whoever removes it owns cleanup
            if let Some((_, old_sess)) = state.sessions.remove(&old_sid) {
                warn!(
                    "Session takeover: evicting old session {} for character {} ({})",
                    old_sid, old_sess.character_name, character_id
                );

                let old_player_id = old_sess.player_id.clone();

                // Save current in-memory state to DB before cleanup.
                // The old handle_socket will skip its save (session already removed),
                // so we must persist here to avoid rolling back to stale DB state.
                if let Some(old_room) = state.rooms.get(&old_sess.room_id) {
                    let old_room_ref = old_room.clone();
                    let played_time_delta = state
                        .play_time_anchors
                        .remove(&character_id)
                        .map(|(_, anchor)| anchor.elapsed().as_secs() as i64)
                        .unwrap_or(0);
                    if let Some(mut save_data) =
                        old_room_ref.get_player_save_data(&old_player_id).await
                    {
                        if save_data.current_map.is_some() {
                            let entrance_positions = state.player_entrance_positions.read().await;
                            if let Some(&(ex, ey)) = entrance_positions.get(&old_player_id) {
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
                                "Session takeover: failed to save character {} before eviction: {}",
                                old_sess.character_name, e
                            );
                        } else {
                            info!(
                                "Session takeover: saved character {} to DB before eviction (played_time +{}s)",
                                old_sess.character_name, played_time_delta
                            );
                        }
                    }

                    // Save quest state, recipes, spells, and slayer
                    if character_id > 0 {
                        if let Some(quest_state) =
                            old_room_ref.get_player_quest_state(&old_player_id).await
                        {
                            if let Err(e) = state
                                .db
                                .save_character_quest_state(character_id, &quest_state)
                                .await
                            {
                                error!(
                                    "Session takeover: failed to save quest state for {}: {}",
                                    old_sess.character_name, e
                                );
                            }
                        }

                        let discovered = old_room_ref
                            .get_player_discovered_recipes(&old_player_id)
                            .await;
                        if let Err(e) = state
                            .db
                            .save_discovered_recipes(character_id, &discovered)
                            .await
                        {
                            error!(
                                "Session takeover: failed to save recipes for {}: {}",
                                old_sess.character_name, e
                            );
                        }

                        let unlocked = old_room_ref
                            .get_player_unlocked_spells(&old_player_id)
                            .await;
                        if let Err(e) = state.db.save_unlocked_spells(character_id, &unlocked).await
                        {
                            error!(
                                "Session takeover: failed to save spells for {}: {}",
                                old_sess.character_name, e
                            );
                        }

                        let slayer_state =
                            old_room_ref.get_player_slayer_state(&old_player_id).await;
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
                                    "Session takeover: failed to save slayer state for {}: {}",
                                    old_sess.character_name, e
                                );
                            }
                        }
                    }
                }

                // Clean up old session state from room
                if let Some(old_room) = state.rooms.get(&old_sess.room_id) {
                    let old_room = old_room.clone();

                    // Clean up instance tracking
                    {
                        use crate::interior::InstanceType;
                        let removed_instance_id =
                            state.player_instances.write().await.remove(&old_player_id);
                        old_room.reset_sync_state(&old_player_id).await;
                        if let Some(instance_id) = removed_instance_id {
                            if let Some(instance) =
                                state.instance_manager.get_by_instance_id(&instance_id)
                            {
                                let other_players: Vec<String> = instance
                                    .get_player_ids()
                                    .await
                                    .into_iter()
                                    .filter(|id| id != &old_player_id)
                                    .collect();
                                let remaining = instance.remove_player(&old_player_id).await;
                                for other_id in &other_players {
                                    old_room
                                        .send_to_player(
                                            other_id,
                                            ServerMessage::PlayerLeft {
                                                id: old_player_id.clone(),
                                            },
                                        )
                                        .await;
                                }
                                if remaining == 0 && instance.instance_type == InstanceType::Private
                                {
                                    if let Some(owner_id) = &instance.owner_id {
                                        state
                                            .instance_manager
                                            .remove_private(owner_id, &instance.map_id);
                                    }
                                }
                            }
                        }
                    }

                    // Clean up entrance positions
                    state
                        .player_entrance_positions
                        .write()
                        .await
                        .remove(&old_player_id);

                    // Unregister player sender (closes old WebSocket send task)
                    old_room.unregister_player_sender(&old_player_id).await;

                    // Notify friends offline
                    old_room
                        .broadcast_friend_status(&old_player_id, false)
                        .await;

                    // Remove from room and notify overworld
                    old_room.remove_player(&old_player_id).await;
                    old_room
                        .send_to_overworld_players(
                            ServerMessage::PlayerLeft {
                                id: old_player_id.clone(),
                            },
                            None,
                        )
                        .await;
                }

                // Clean up play time anchor (already consumed during save above, but
                // remove defensively in case save was skipped)
                state.play_time_anchors.remove(&character_id);

                // Mark offline (will be re-marked online when new socket connects)
                state.online_characters.remove(&character_id);
            }
        } else {
            // Character marked online but no session found — clean up stale state
            warn!(
                "Session takeover: character {} marked online but no session found, cleaning up",
                character_id
            );
            state.online_characters.remove(&character_id);
        }
    }

    let room = state.get_or_create_room(&room_name).await;
    let room_id = room.id.clone();

    // Create session for this character
    let session_id = Uuid::new_v4().to_string();
    let player_id = format!("char_{}", character_id);

    // NOTE: We do NOT mark the character as online here. That happens when the
    // WebSocket actually connects (in handle_socket or spectator upgrade).
    // This prevents orphaned online_characters entries if the client never connects.

    // Reserve the session with character info
    state.sessions.insert(
        session_id.clone(),
        GameSession {
            room_id: room_id.clone(),
            player_id: player_id.clone(),
            character_name: character_data.name.clone(),
            character_id,
            account_id,
            auth_token: auth_token.clone(),
            current_map: character_data.current_map.clone(),
            entrance_x: character_data.entrance_x,
            entrance_y: character_data.entrance_y,
            is_new_character: character_data.played_time == 0,
            command_gate: Arc::new(RwLock::new(true)),
        },
    );

    info!(
        "Tutorial: character '{}' played_time={}, is_new_character={}",
        character_data.name,
        character_data.played_time,
        character_data.played_time == 0
    );

    // Start tracking play time for this character
    state
        .play_time_anchors
        .insert(character_id, std::time::Instant::now());

    // Load saved character into the game room
    info!(
        "Loading character: {} (id: {}) at ({}, {}) as {} {}",
        character_data.name,
        character_id,
        character_data.x,
        character_data.y,
        character_data.gender,
        character_data.skin
    );

    room.reserve_player_with_data(
        &player_id,
        &character_data.name,
        character_data.x as i32,
        character_data.y as i32,
        character_data.z,
        character_data.hp,
        character_data.prayer_points,
        character_data.mp,
        character_data.skills.clone(),
        character_data.gold,
        &character_data.inventory_json,
        &character_data.gender,
        &character_data.skin,
        character_data.hair_style,
        character_data.hair_color,
        character_data.equipped_head.clone(),
        character_data.equipped_body.clone(),
        character_data.equipped_weapon.clone(),
        character_data.equipped_back.clone(),
        character_data.equipped_feet.clone(),
        character_data.equipped_ring.clone(),
        character_data.equipped_gloves.clone(),
        character_data.equipped_necklace.clone(),
        character_data.equipped_belt.clone(),
        character_data.is_admin,
        account_id,
        Some(client_ip.clone()),
        character_data.sitting_at_x,
        character_data.sitting_at_y,
        &character_data.bank_json,
        character_data.bank_gold,
        character_data.bank_max_slots,
        &character_data.combat_style_prefs,
    )
    .await;

    // Load quest state from database
    match state.db.load_character_quest_state(character_id).await {
        Ok(quest_state) => {
            let active_count = quest_state.active_quests.len();
            let completed_count = quest_state.completed_quests.len();
            room.set_player_quest_state(&player_id, quest_state).await;
            if active_count > 0 || completed_count > 0 {
                info!(
                    "Loaded quest state for {}: {} active, {} completed",
                    character_data.name, active_count, completed_count
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load quest state for character {}: {}",
                character_id,
                e
            );
            // Continue with empty quest state (default)
        }
    }

    // Load discovered recipes from database
    match state.db.load_discovered_recipes(character_id).await {
        Ok(recipes) => {
            let count = recipes.len();
            let recipe_set: std::collections::HashSet<String> = recipes.into_iter().collect();
            room.set_player_discovered_recipes(&player_id, recipe_set)
                .await;
            if count > 0 {
                info!(
                    "Loaded {} discovered recipes for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load discovered recipes for character {}: {}",
                character_id,
                e
            );
        }
    }

    // Load unlocked spells from database
    match state.db.load_unlocked_spells(character_id).await {
        Ok(spells) => {
            let count = spells.len();
            let spell_set: std::collections::HashSet<String> = spells.into_iter().collect();
            room.set_player_unlocked_spells(&player_id, spell_set).await;
            if count > 0 {
                info!(
                    "Loaded {} unlocked spells for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load unlocked spells for character {}: {}",
                character_id,
                e
            );
        }
    }

    // Load collection log from database
    match state.db.load_collection_log(character_id).await {
        Ok(entries) => {
            let count = entries.len();
            let log_set: std::collections::HashSet<(String, String)> = entries
                .iter()
                .map(|(item_id, source, _, _)| (item_id.clone(), source.clone()))
                .collect();
            room.set_player_collection_log(&player_id, log_set).await;
            if count > 0 {
                info!(
                    "Loaded {} collection log entries for {}",
                    count, character_data.name
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load collection log for character {}: {}",
                character_id,
                e
            );
        }
    }

    // Load slayer state from database
    let slayer_state = state
        .db
        .load_character_slayer_state(character_id)
        .await
        .unwrap_or_default();
    room.set_player_slayer_state(&player_id, slayer_state.clone())
        .await;
    if slayer_state.current_task.is_some() || slayer_state.tasks_completed > 0 {
        info!(
            "Loaded slayer state for {}: {} tasks completed, {} points",
            character_data.name, slayer_state.tasks_completed, slayer_state.points
        );
    }

    // Load active title from database
    match state.db.get_active_title(character_id).await {
        Ok(Some(title_id)) => {
            if let Some(title_text) = crate::game::titles::title_display(&title_id) {
                room.set_player_active_title(&player_id, Some(title_text.to_string()))
                    .await;
                info!(
                    "Loaded active title for {}: {}",
                    character_data.name, title_text
                );
            }
        }
        Ok(None) => {} // No title set
        Err(e) => {
            tracing::warn!(
                "Failed to load active title for character {}: {}",
                character_id,
                e
            );
        }
    }

    let client_count = room.player_count().await;

    // Generate signed session token for WebSocket upgrade
    let session_token = state.token_signer.create_token(&session_id, &room_id);

    info!(
        "Matchmaking: room={}, character={} (id: {})",
        room_id, character_data.name, character_id
    );

    Json(MatchmakeResponse {
        room: RoomInfo {
            room_id,
            name: room_name,
            clients: client_count,
        },
        session_token,
    })
    .into_response()
}
