use super::*;

impl AppState {
    pub(super) async fn new(log_buffer: log_buffer::LogBuffer, config: Arc<ServerConfig>) -> Self {
        // Initialize database
        let db = Database::new(&config.database_url)
            .await
            .expect("Failed to initialize database");

        // Load entity registry from TOML files
        let mut entity_registry = EntityRegistry::new();
        let data_dir = std::path::Path::new("data");
        if let Err(e) = entity_registry.load_from_directory(data_dir) {
            error!("Failed to load entity registry: {}", e);
        }

        // Load item registry from TOML files
        let mut item_registry = ItemRegistry::new();
        if let Err(e) = item_registry.load_from_directory(data_dir) {
            error!("Failed to load item registry: {}", e);
        }

        // Load prayer registry from TOML file
        let mut prayer_registry = PrayerRegistry::new();
        if let Err(e) = prayer_registry.load_from_directory(data_dir) {
            error!("Failed to load prayer registry: {}", e);
        }

        // Load quest registry from TOML files
        let quest_registry = Arc::new(QuestRegistry::new(data_dir));
        if let Err(e) = quest_registry.load_all().await {
            error!("Failed to load quest registry: {}", e);
        }

        // Load crafting registry from TOML files
        let mut crafting_registry = CraftingRegistry::new();
        if let Err(e) = crafting_registry.load_from_directory(data_dir) {
            error!("Failed to load crafting registry: {}", e);
        }

        // Load chest registry from TOML file
        let mut chest_registry = crate::chest::ChestRegistry::new();
        chest_registry.load_from_file(&data_dir.join("chests.toml"));

        // Load collection log definitions from TOML file
        let collection_log_defs =
            collection_log::CollectionLogDefinitions::load("data/collection_log.toml");

        // Build display name lookup for collection log subcategories
        let quest_names: std::collections::HashMap<String, String> = {
            let quests = quest_registry.all_quests().await;
            quests
                .into_iter()
                .map(|q| (q.id.clone(), q.name.clone()))
                .collect()
        };
        let collection_log_display_names: Vec<(String, String)> = collection_log_defs
            .build_display_names(&entity_registry, &quest_names)
            .into_iter()
            .collect();

        // Load interior registry from JSON files
        let interior_registry = Arc::new(
            InteriorRegistry::load_from_directory("maps/interiors")
                .expect("Failed to load interior registry"),
        );

        // Initialize instance manager
        let instance_manager = Arc::new(InstanceManager::new());

        // Start hot-reload watcher for quest files (dev mode)
        #[cfg(debug_assertions)]
        {
            match quest_registry.start_file_watcher() {
                Ok(mut rx) => {
                    // Spawn task to log reload events
                    tokio::spawn(async move {
                        while let Some(event) = rx.recv().await {
                            match event {
                                quest::HotReloadEvent::Reloaded(path) => {
                                    info!("Quest hot-reload: {}", path);
                                }
                                quest::HotReloadEvent::Error(e) => {
                                    error!("Quest hot-reload error: {}", e);
                                }
                            }
                        }
                    });
                    info!("Quest hot-reload enabled");
                }
                Err(e) => {
                    warn!("Failed to start quest hot-reload: {}", e);
                }
            }
        }

        Self {
            config: config.clone(),
            rooms: Arc::new(DashMap::new()),
            room_creation_lock: Arc::new(Mutex::new(())),
            sessions: Arc::new(DashMap::new()),
            auth_sessions: Arc::new(DashMap::new()),
            db: Arc::new(db),
            // Auth: 10 attempts per 60 seconds per IP
            auth_rate_limiter: RateLimiter::new(10, 60),
            // Matchmaking: 20 attempts per 60 seconds per IP
            matchmake_rate_limiter: RateLimiter::new(20, 60),
            // SECURITY: Token signer for session tokens
            token_signer: SessionTokenSigner::new(config.session_signing_secret.clone()),
            entity_registry: Arc::new(entity_registry),
            item_registry: Arc::new(item_registry),
            prayer_registry: Arc::new(prayer_registry),
            quest_registry,
            crafting_registry: Arc::new(crafting_registry),
            chest_registry: Arc::new(chest_registry),
            collection_log_defs: Arc::new(collection_log_defs),
            collection_log_display_names: Arc::new(collection_log_display_names),
            interior_registry,
            instance_manager,
            player_instances: Arc::new(RwLock::new(HashMap::new())),
            player_entrance_positions: Arc::new(RwLock::new(HashMap::new())),
            play_time_anchors: Arc::new(DashMap::new()),
            online_characters: Arc::new(DashSet::new()),
            character_session_locks: Arc::new(DashMap::new()),
            log_buffer,
            perf_metrics: perf_metrics::PerfMetrics::new(),
            leaderboard_cache: Arc::new(RwLock::new(LeaderboardCache::default())),
        }
    }

    pub(super) async fn get_or_create_room(&self, room_name: &str) -> Arc<GameRoom> {
        for room in self.rooms.iter() {
            if room.name == room_name {
                return room.clone();
            }
        }

        let _creation_guard = self.room_creation_lock.lock().await;

        for room in self.rooms.iter() {
            if room.name == room_name {
                return room.clone();
            }
        }

        // Create new room and store by its UUID
        let room = Arc::new(
            GameRoom::new(
                room_name,
                self.entity_registry.clone(),
                self.quest_registry.clone(),
                self.crafting_registry.clone(),
                self.item_registry.clone(),
                self.prayer_registry.clone(),
                self.player_instances.clone(),
                self.instance_manager.clone(),
                Some(self.db.clone()),
                self.interior_registry.clone(),
                self.chest_registry.clone(),
            )
            .await,
        );
        room.init_top_level_player().await;
        self.rooms.insert(room.id.clone(), room.clone());
        room
    }
}

pub(super) struct SessionLease {
    _command_guard: OwnedRwLockReadGuard<bool>,
}

pub(super) async fn acquire_session_lease(
    sessions: &DashMap<String, GameSession>,
    auth_sessions: &AuthSessions,
    session_id: &str,
    room_id: &str,
    player_id: &str,
) -> Option<SessionLease> {
    let session = sessions.get(session_id).map(|entry| entry.clone())?;
    if session.room_id != room_id || session.player_id != player_id {
        return None;
    }

    let command_guard = session.command_gate.clone().read_owned().await;
    if !*command_guard {
        return None;
    }

    let current_session = sessions.get(session_id)?;
    if current_session.player_id != player_id
        || !Arc::ptr_eq(&current_session.command_gate, &session.command_gate)
    {
        return None;
    }

    let auth_session = get_auth_session(auth_sessions, &session.auth_token)?;
    if auth_session.account_id != session.account_id {
        return None;
    }

    Some(SessionLease {
        _command_guard: command_guard,
    })
}

#[cfg(test)]
mod session_lease_tests {
    use super::*;

    fn test_session(command_gate: Arc<RwLock<bool>>) -> GameSession {
        GameSession {
            room_id: "room".to_string(),
            player_id: "char_42".to_string(),
            character_name: "Test".to_string(),
            character_id: 42,
            account_id: 7,
            auth_token: "auth".to_string(),
            current_map: None,
            entrance_x: None,
            entrance_y: None,
            is_new_character: false,
            command_gate,
        }
    }

    #[tokio::test]
    async fn takeover_waits_for_in_flight_command_and_rejects_stale_session() {
        let sessions = DashMap::new();
        let auth_sessions = Arc::new(DashMap::new());
        auth_sessions.insert(
            "auth".to_string(),
            AuthSession::new(7, "tester".to_string(), Duration::from_secs(60)),
        );

        let old_gate = Arc::new(RwLock::new(true));
        sessions.insert("old".to_string(), test_session(old_gate.clone()));

        let old_lease = acquire_session_lease(&sessions, &auth_sessions, "old", "room", "char_42")
            .await
            .expect("old session should initially own the player");

        let invalidation = tokio::spawn(async move {
            let mut active = old_gate.write().await;
            *active = false;
        });
        tokio::task::yield_now().await;
        assert!(
            !invalidation.is_finished(),
            "takeover must wait for the in-flight command"
        );

        drop(old_lease);
        invalidation.await.unwrap();
        sessions.remove("old");

        assert!(
            acquire_session_lease(&sessions, &auth_sessions, "old", "room", "char_42")
                .await
                .is_none(),
            "the stale socket must not receive another command lease"
        );

        sessions.insert("new".to_string(), test_session(Arc::new(RwLock::new(true))));
        assert!(
            acquire_session_lease(&sessions, &auth_sessions, "new", "room", "char_42")
                .await
                .is_some(),
            "the replacement session should own the player"
        );
    }
}
