use super::*;

impl GameRoom {
    pub async fn new(
        name: &str,
        entity_registry: Arc<EntityRegistry>,
        quest_registry: Arc<QuestRegistry>,
        crafting_registry: Arc<crate::crafting::CraftingRegistry>,
        item_registry: Arc<ItemRegistry>,
        prayer_registry: Arc<PrayerRegistry>,
        player_instances: Arc<RwLock<HashMap<String, String>>>,
        instance_manager: Arc<crate::instance::InstanceManager>,
        db: Option<Arc<crate::db::Database>>,
        interior_registry: Arc<crate::interior::InteriorRegistry>,
        chest_registry: Arc<crate::chest::ChestRegistry>,
    ) -> Self {
        let (tx, _) = broadcast::channel(256);
        let world = Arc::new(World::new("maps/world_0"));
        let (spawn_x, spawn_y) = world.get_spawn_position().await;
        let spawn_chunk = ChunkCoord::from_world(spawn_x, spawn_y);
        tracing::info!(
            "Preloading overworld chunks around spawn ({}, {}) at chunk ({}, {}) radius {}",
            spawn_x,
            spawn_y,
            spawn_chunk.x,
            spawn_chunk.y,
            SPAWN_PRELOAD_RADIUS
        );
        world
            .preload_chunks(spawn_chunk, SPAWN_PRELOAD_RADIUS)
            .await;

        // Create quest runner with the registry
        let quest_runner = Arc::new(QuestRunner::new(quest_registry.clone()));

        // Load all chunks and spawn NPCs from entity_spawns
        let mut npcs = HashMap::new();
        let mut npc_counter = 0u32;

        // Discover all chunk files and load entities from each
        let chunk_coords = world.discover_chunk_coords();
        tracing::info!("Discovered {} chunk files", chunk_coords.len());

        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for spawn in &chunk.entity_spawns {
                    let npc_id = spawn
                        .unique_id
                        .clone()
                        .unwrap_or_else(|| format!("npc_{}", npc_counter));
                    npc_counter += 1;

                    if let Some(prototype) = entity_registry.get(&spawn.entity_id) {
                        // Use spawn's level if specified, otherwise use prototype's level
                        let level = spawn.level.unwrap_or(prototype.stats.level);
                        tracing::info!(
                            "Spawning {} at ({}, {}) level {}",
                            spawn.entity_id,
                            spawn.world_x,
                            spawn.world_y,
                            level
                        );
                        let npc = Npc::from_prototype(
                            &npc_id,
                            &spawn.entity_id,
                            prototype,
                            spawn.world_x,
                            spawn.world_y,
                            level,
                            spawn.facing.as_deref(),
                        );
                        npcs.insert(npc_id, npc);
                    } else {
                        tracing::warn!("Prototype '{}' not found, skipping spawn", spawn.entity_id);
                    }
                }
            }
        }

        tracing::info!("Spawned {} NPCs from chunk entity_spawns", npcs.len());

        // Load shop registry
        let mut shop_registry = ShopRegistry::new();
        if let Err(e) = shop_registry.load_from_directory(std::path::Path::new("data/shops")) {
            tracing::error!("Failed to load shop registry: {}", e);
        }
        tracing::info!("Loaded {} shop definitions", shop_registry.len());

        // Load gathering system
        let mut gathering =
            match crate::gathering::GatheringSystem::load(std::path::Path::new("data")) {
                Ok(g) => {
                    tracing::info!("Loaded gathering system with {} zones", g.zones.len());
                    g
                }
                Err(e) => {
                    tracing::warn!("Failed to load gathering system: {} (using empty)", e);
                    crate::gathering::GatheringSystem::new()
                }
            };

        // Load gathering markers from chunk data
        let mut chunk_marker_count = 0;
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for gz in &chunk.gathering_zones {
                    gathering.add_marker(crate::gathering::GatheringMarker {
                        x: gz.world_x,
                        y: gz.world_y,
                        zone_id: gz.zone_id.clone(),
                    });
                    chunk_marker_count += 1;
                }
            }
        }
        if chunk_marker_count > 0 {
            tracing::info!(
                "Loaded {} gathering markers from chunk data",
                chunk_marker_count
            );
        }

        // Cache portal tile positions (immutable, computed once at startup)
        let mut portal_tiles = std::collections::HashSet::new();
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                let base_x = coord.x * crate::chunk::CHUNK_SIZE as i32;
                let base_y = coord.y * crate::chunk::CHUNK_SIZE as i32;
                for portal in &chunk.portals {
                    for dx in 0..portal.width {
                        for dy in 0..portal.height {
                            portal_tiles.insert((base_x + portal.x + dx, base_y + portal.y + dy));
                        }
                    }
                }
            }
        }
        if !portal_tiles.is_empty() {
            tracing::info!(
                "Cached {} portal tiles for NPC collision",
                portal_tiles.len()
            );
        }

        // Load quest locations for reach_location objectives
        let quest_locations: HashMap<String, QuestLocation> =
            match std::fs::read_to_string("data/quest_locations.toml") {
                Ok(content) => match toml::from_str::<HashMap<String, QuestLocation>>(&content) {
                    Ok(locs) => {
                        tracing::info!("Loaded {} quest locations", locs.len());
                        locs
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse quest_locations.toml: {}", e);
                        HashMap::new()
                    }
                },
                Err(_) => {
                    tracing::info!("No quest_locations.toml found, skipping");
                    HashMap::new()
                }
            };

        // Load woodcutting system
        let woodcutting =
            match crate::woodcutting::WoodcuttingSystem::load(std::path::Path::new("data")) {
                Ok(w) => {
                    tracing::info!(
                        "Loaded woodcutting system with {} tree types",
                        w.tree_types.len()
                    );
                    w
                }
                Err(e) => {
                    tracing::warn!("Failed to load woodcutting system: {} (using empty)", e);
                    crate::woodcutting::WoodcuttingSystem::new()
                }
            };

        // Load mining system
        let mining = match crate::mining::MiningSystem::load(std::path::Path::new("data")) {
            Ok(m) => {
                tracing::info!("Loaded mining system with {} ore types", m.ore_types.len());
                m
            }
            Err(e) => {
                tracing::warn!("Failed to load mining system: {} (using empty)", e);
                crate::mining::MiningSystem::new()
            }
        };

        // Load farming system
        let mut farming = match crate::farming::FarmingSystem::load(std::path::Path::new("data")) {
            Ok(f) => {
                tracing::info!(
                    "Loaded farming system with {} crops, {} patches",
                    f.crops.len(),
                    f.patches.len()
                );
                f
            }
            Err(e) => {
                tracing::warn!("Failed to load farming system: {} (using empty)", e);
                crate::farming::FarmingSystem::new()
            }
        };
        let mut resource_contracts = crate::resource_contracts::ResourceContractManager::new();

        // Restore planted patches from database
        if let Some(ref db) = db {
            match db.load_farming_patches().await {
                Ok(saved_patches) => {
                    let count = saved_patches.len();
                    for (patch_id, player_id, crop_id, planted_at) in saved_patches {
                        farming.restore_patch(&patch_id, &player_id, &crop_id, planted_at);
                    }
                    if count > 0 {
                        tracing::info!("Restored {} planted farming patches from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load farming patches from database: {}", e);
                }
            }
        }

        // Load plot unlocks from database
        if let Some(ref db) = db {
            match db.load_plot_unlocks().await {
                Ok(unlocks) => {
                    let count = unlocks.len();
                    for (player_id, plot_id) in &unlocks {
                        farming.restore_plot_unlock(player_id, *plot_id);
                    }
                    if count > 0 {
                        tracing::info!("Restored {} farming plot unlocks from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load farming plot unlocks from database: {}", e);
                }
            }
        }

        // Load shared resource contracts from database, then migrate any legacy farming contracts.
        if let Some(ref db) = db {
            match db.load_resource_contracts().await {
                Ok(contracts) => {
                    let count = contracts.len();
                    for (
                        player_id,
                        contract_kind,
                        difficulty,
                        target_item_id,
                        target_name,
                        amount_required,
                        amount_completed,
                        giver_npc_id,
                        giver_name,
                        created_at,
                    ) in &contracts
                    {
                        resource_contracts.restore_contract(
                            player_id,
                            contract_kind,
                            difficulty,
                            target_item_id,
                            target_name,
                            *amount_required,
                            *amount_completed,
                            giver_npc_id,
                            giver_name,
                            *created_at,
                        );
                    }
                    if count > 0 {
                        tracing::info!("Restored {} resource contracts from database", count);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load resource contracts from database: {}", e);
                }
            }

            match db.load_farming_contracts().await {
                Ok(contracts) => {
                    let mut migrated = 0usize;
                    for (
                        player_id,
                        difficulty,
                        crop_id,
                        amount_required,
                        amount_harvested,
                        created_at,
                    ) in contracts
                    {
                        if resource_contracts.has_contract(&player_id) {
                            if let Err(e) = db.delete_farming_contract(&player_id).await {
                                tracing::warn!(
                                    "Failed to delete legacy farming contract for {}: {}",
                                    player_id,
                                    e
                                );
                            }
                            continue;
                        }

                        let Some(difficulty) =
                            crate::resource_contracts::ContractDifficulty::from_str(&difficulty)
                        else {
                            continue;
                        };

                        let target_item_id = farming
                            .crops
                            .get(&crop_id)
                            .map(|crop| crop.produce_item.clone())
                            .unwrap_or(crop_id);
                        let target_name = item_registry
                            .get(&target_item_id)
                            .map(|item| item.display_name.clone())
                            .unwrap_or_else(|| target_item_id.clone());

                        let contract = crate::resource_contracts::ResourceContract {
                            player_id: player_id.clone(),
                            kind: crate::resource_contracts::ResourceContractKind::Farming,
                            difficulty,
                            target_item_id: target_item_id.clone(),
                            target_name: target_name.clone(),
                            amount_required,
                            amount_completed: amount_harvested,
                            created_at,
                            giver_npc_id: "master_farmer".to_string(),
                            giver_name: "Master Farmer".to_string(),
                        };

                        resource_contracts.insert_contract(contract.clone());
                        migrated += 1;

                        if let Err(e) = db
                            .save_resource_contract(
                                &player_id,
                                contract.kind.as_str(),
                                contract.difficulty.as_str(),
                                &target_item_id,
                                &target_name,
                                amount_required,
                                amount_harvested,
                                "master_farmer",
                                "Master Farmer",
                                created_at,
                            )
                            .await
                        {
                            tracing::warn!(
                                "Failed to migrate legacy farming contract for {}: {}",
                                player_id,
                                e
                            );
                        }

                        if let Err(e) = db.delete_farming_contract(&player_id).await {
                            tracing::warn!(
                                "Failed to delete legacy farming contract for {}: {}",
                                player_id,
                                e
                            );
                        }
                    }

                    if migrated > 0 {
                        tracing::info!(
                            "Migrated {} legacy farming contracts into resource contracts",
                            migrated
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load legacy farming contracts from database: {}",
                        e
                    );
                }
            }
        }

        // Load slayer registry
        let slayer_registry =
            match crate::slayer::SlayerRegistry::load(std::path::Path::new("data")) {
                Ok(r) => Arc::new(r),
                Err(e) => {
                    tracing::warn!(
                        "Failed to load slayer registry: {}, using empty registry",
                        e
                    );
                    Arc::new(crate::slayer::SlayerRegistry::empty())
                }
            };

        // Load chair config
        let mut chair_gids: HashMap<u32, Direction> = HashMap::new();
        match std::fs::read_to_string("data/chairs.toml") {
            Ok(content) => match toml::from_str::<ChairsConfig>(&content) {
                Ok(config) => {
                    for entry in config.chairs {
                        let dir = match entry.direction.as_str() {
                            "down" => Direction::Down,
                            "left" => Direction::Left,
                            "up" => Direction::Up,
                            "right" => Direction::Right,
                            _ => Direction::Down,
                        };
                        chair_gids.insert(entry.gid, dir);
                    }
                    tracing::info!("Loaded {} chair GID definitions", chair_gids.len());
                }
                Err(e) => tracing::warn!("Failed to parse chairs.toml: {}", e),
            },
            Err(e) => tracing::warn!("Failed to read chairs.toml: {} (no chairs)", e),
        }

        // Populate chair positions from chunk map objects
        let mut chairs: HashMap<(i32, i32), ChairState> = HashMap::new();
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for obj in &chunk.objects {
                    if let Some(&dir) = chair_gids.get(&obj.gid) {
                        chairs.insert(
                            (obj.tile_x, obj.tile_y),
                            ChairState {
                                direction: dir,
                                occupied_by: None,
                            },
                        );
                    }
                }
            }
        }
        if !chairs.is_empty() {
            tracing::info!("Found {} chairs on the map", chairs.len());
        }

        // Load scroll spell registry
        let mut scroll_spell_registry = crate::scroll_spell::ScrollSpellRegistry::new();
        let scroll_spells_path = std::path::Path::new("data/spells/scroll_spells.toml");
        if scroll_spells_path.exists() {
            if let Err(e) = scroll_spell_registry.load_from_file(scroll_spells_path) {
                tracing::error!("Failed to load scroll spell registry: {}", e);
            }
        }

        // Load persistent ground spawn definitions and create initial ground items
        let mut ground_spawn_manager =
            crate::ground_spawn::GroundSpawnManager::load(std::path::Path::new("data"));
        let mut ground_items = HashMap::new();
        {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let initial_spawns = ground_spawn_manager.get_initial_spawns();
            for (spawn_id, item_id, x, y, quantity, instance_id) in initial_spawns {
                let ground_item_id = format!("persistent_{}", spawn_id);
                let ground_item = crate::item::GroundItem::new_in_instance(
                    &ground_item_id,
                    &item_id,
                    x,
                    y,
                    quantity,
                    None,
                    current_time,
                    instance_id,
                );
                ground_spawn_manager.set_active_ground_item(&spawn_id, ground_item_id.clone());
                ground_items.insert(ground_item_id, ground_item);
            }
            if !ground_items.is_empty() {
                tracing::info!(
                    "Created {} persistent ground items from spawns",
                    ground_items.len()
                );
            }
        }

        // Load overworld chest spawns from TOML
        let overworld_chest_spawns = {
            let spawns_path = std::path::Path::new("data/chest_spawns.toml");
            if spawns_path.exists() {
                let content = std::fs::read_to_string(spawns_path).unwrap_or_default();
                let file: crate::chest::ChestSpawnsFile =
                    toml::from_str(&content).unwrap_or_else(|e| {
                        tracing::warn!("Failed to parse chest_spawns.toml: {}", e);
                        crate::chest::ChestSpawnsFile { chests: Vec::new() }
                    });
                file.chests
            } else {
                Vec::new()
            }
        };

        // Collect interior chest placements from interior_registry
        let mut interior_chests = Vec::new();
        for id in interior_registry.list_ids() {
            if let Some(interior) = interior_registry.get(id) {
                tracing::debug!(
                    "Interior '{}' has {} chests defined",
                    id,
                    interior.chests.len()
                );
                for chest_spawn in &interior.chests {
                    tracing::info!(
                        "Interior '{}' chest: {} at ({}, {})",
                        id,
                        chest_spawn.chest_id,
                        chest_spawn.x,
                        chest_spawn.y
                    );
                    interior_chests.push((
                        id.clone(),
                        chest_spawn.chest_id.clone(),
                        chest_spawn.x,
                        chest_spawn.y,
                    ));
                }
            }
        }
        tracing::info!(
            "Collected {} interior chest placements",
            interior_chests.len()
        );

        // Create ChestManager and load saved data
        let mut chest_manager = crate::chest::ChestManager::new();
        chest_manager.init_from_registry(
            &chest_registry,
            &overworld_chest_spawns,
            &interior_chests,
        );
        if let Some(ref db) = db {
            match db.load_all_chests().await {
                Ok(saved) => {
                    chest_manager.load_saved_data(&saved);
                    tracing::info!("Loaded {} saved chest states", saved.len());
                }
                Err(e) => tracing::warn!("Failed to load chest data: {}", e),
            }
        }

        let waystone_manager = crate::waystone::WaystoneManager::load(std::path::Path::new("data"));
        let overworld_world_map = world_map::build_overworld_world_map(
            &world,
            &chunk_coords,
            &npcs,
            &chest_registry,
            &overworld_chest_spawns,
            &waystone_manager,
        )
        .await;

        // Load PVP zone allowlist
        let pvp_zones: HashSet<(i32, i32)> = match std::fs::read_to_string("data/pvp_zones.toml") {
            Ok(content) => {
                #[derive(serde::Deserialize)]
                struct PvpZoneConfig {
                    #[serde(default)]
                    zones: Vec<PvpZoneEntry>,
                }
                #[derive(serde::Deserialize)]
                struct PvpZoneEntry {
                    chunk_x: i32,
                    chunk_y: i32,
                }
                match toml::from_str::<PvpZoneConfig>(&content) {
                    Ok(config) => {
                        let set: HashSet<(i32, i32)> = config
                            .zones
                            .iter()
                            .map(|z| (z.chunk_x, z.chunk_y))
                            .collect();
                        tracing::info!("Loaded {} PVP zone chunks", set.len());
                        set
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse pvp_zones.toml: {}", e);
                        HashSet::new()
                    }
                }
            }
            Err(_) => {
                tracing::info!("No pvp_zones.toml found, PVP disabled everywhere");
                HashSet::new()
            }
        };

        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            players: RwLock::new(HashMap::new()),
            npcs: RwLock::new(npcs),
            ground_items: RwLock::new(ground_items),
            visible_ground_items: RwLock::new(HashMap::new()),
            world,
            entity_registry,
            quest_registry,
            quest_runner,
            player_quest_states: RwLock::new(HashMap::new()),
            crafting_registry,
            item_registry,
            prayer_registry,
            shop_registry: RwLock::new(shop_registry),
            last_shop_restock: RwLock::new(std::time::Instant::now()),
            player_chunks: RwLock::new(HashMap::new()),
            tick: RwLock::new(0),
            broadcast_tx: tx,
            player_senders: RwLock::new(HashMap::new()),
            sync_states: DashMap::new(),
            player_instances,
            npc_interaction_grants: RwLock::new(HashMap::new()),
            dialogue_grants: RwLock::new(HashMap::new()),
            instance_manager,
            arena_manager: RwLock::new(crate::arena::ArenaManager::new(
                crate::arena::ArenaConfig::default(),
            )),
            koth_states: RwLock::new(std::collections::HashMap::new()),
            boss_states: RwLock::new(std::collections::HashMap::new()),
            pharaoh_boss_states: RwLock::new(HashMap::new()),
            db,
            gathering: RwLock::new(gathering),
            woodcutting: RwLock::new(woodcutting),
            mining: RwLock::new(mining),
            chair_gids,
            chairs: RwLock::new(chairs),
            farming: RwLock::new(farming),
            resource_contracts: RwLock::new(resource_contracts),
            portal_tiles,
            quest_locations,
            slayer_registry,
            player_slayer_states: RwLock::new(HashMap::new()),
            interior_registry,
            scroll_spell_registry: Arc::new(scroll_spell_registry),
            ground_spawn_manager: RwLock::new(ground_spawn_manager),
            dig_site_manager: RwLock::new(crate::dig_site::DigSiteManager::load(
                std::path::Path::new("data"),
            )),
            waystone_manager: RwLock::new(waystone_manager),
            chest_registry,
            chest_manager: RwLock::new(chest_manager),
            player_open_chests: RwLock::new(HashMap::new()),
            spectator_senders: RwLock::new(HashMap::new()),
            trades: RwLock::new(HashMap::new()),
            player_trades: RwLock::new(HashMap::new()),
            trade_requests: RwLock::new(HashMap::new()),
            overworld_world_map,
            pvp_zones,
            movement_anomalies: MovementAnomalyCounters::default(),
            crafting_order_registry: crafting_orders::CraftingOrderRegistry::load("data"),
            crate_loot_registry: crate_loot::CrateLootRegistry::load("data"),
            top_level_player_name: RwLock::new(None),
            top_level_value: RwLock::new(0),
            second_level_player_name: RwLock::new(None),
            second_level_value: RwLock::new(0),
        }
    }
}
