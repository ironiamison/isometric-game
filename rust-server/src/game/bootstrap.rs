use super::*;

impl GameRoom {
    pub async fn new(
        name: &str,
        content: Arc<crate::content::ContentRegistries>,
        player_instances: Arc<RwLock<HashMap<String, String>>>,
        instance_manager: Arc<crate::instance::InstanceManager>,
        db: Option<Arc<crate::db::Database>>,
    ) -> Self {
        let entity_registry = content.entity_registry.clone();
        let quest_registry = content.quest_registry.clone();
        let crafting_registry = content.crafting_registry.clone();
        let item_registry = content.item_registry.clone();
        let prayer_registry = content.prayer_registry.clone();
        let interior_registry = content.interior_registry.clone();
        let chest_registry = content.chest_registry.clone();
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
            let chunk = world.get_or_load_chunk(*coord).await.unwrap_or_else(|| {
                panic!(
                    "failed to load authoritative chunk ({}, {})",
                    coord.x, coord.y
                )
            });
            for portal in &chunk.portals {
                if portal.target_map == "overworld" {
                    continue;
                }
                let target = interior_registry
                    .get(&portal.target_map)
                    .unwrap_or_else(|| {
                        panic!(
                            "chunk ({}, {}) portal '{}' references unknown map '{}'",
                            coord.x, coord.y, portal.id, portal.target_map
                        )
                    });
                if !portal.target_spawn.is_empty()
                    && target.get_spawn_point(&portal.target_spawn).is_none()
                {
                    panic!(
                        "chunk ({}, {}) portal '{}' references unknown spawn '{}' in '{}'",
                        coord.x, coord.y, portal.id, portal.target_spawn, portal.target_map
                    );
                }
            }
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
                    if npcs.insert(npc_id.clone(), npc).is_some() {
                        panic!("duplicate NPC unique ID '{npc_id}'");
                    }
                } else {
                    panic!(
                        "chunk ({}, {}) references unknown entity '{}'",
                        coord.x, coord.y, spawn.entity_id
                    );
                }
            }
        }

        tracing::info!("Spawned {} NPCs from chunk entity_spawns", npcs.len());

        // Load shop registry
        let mut shop_registry = ShopRegistry::new();
        shop_registry
            .load_from_directory(std::path::Path::new("data/shops"))
            .unwrap_or_else(|error| panic!("shop registry validation failed: {error}"));
        tracing::info!("Loaded {} shop definitions", shop_registry.len());

        // Load gathering system
        let mut gathering = crate::gathering::GatheringSystem::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("gathering content validation failed: {error}"));
        for (interior_id, interior) in interior_registry.iter() {
            for marker in &interior.gathering_zones {
                if !gathering.zones.contains_key(&marker.zone_id) {
                    panic!(
                        "interior '{interior_id}' references unknown gathering zone '{}'",
                        marker.zone_id
                    );
                }
            }
        }
        tracing::info!(
            "Loaded gathering system with {} zones",
            gathering.zones.len()
        );

        // Load gathering markers from chunk data
        let mut chunk_marker_count = 0;
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for gz in &chunk.gathering_zones {
                    if !gathering.zones.contains_key(&gz.zone_id) {
                        panic!(
                            "chunk ({}, {}) references unknown gathering zone '{}'",
                            coord.x, coord.y, gz.zone_id
                        );
                    }
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
        let quest_locations_source = std::fs::read_to_string("data/quest_locations.toml")
            .unwrap_or_else(|error| panic!("failed to read quest_locations.toml: {error}"));
        let quest_locations: HashMap<String, QuestLocation> =
            toml::from_str(&quest_locations_source)
                .unwrap_or_else(|error| panic!("invalid quest_locations.toml: {error}"));
        tracing::info!("Loaded {} quest locations", quest_locations.len());

        // Load woodcutting system
        let woodcutting = crate::woodcutting::WoodcuttingSystem::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("woodcutting content validation failed: {error}"));
        tracing::info!(
            "Loaded woodcutting system with {} tree types",
            woodcutting.tree_types.len()
        );

        // Load mining system
        let mining = crate::mining::MiningSystem::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("mining content validation failed: {error}"));
        tracing::info!(
            "Loaded mining system with {} ore types",
            mining.ore_types.len()
        );

        // Load farming system
        let mut farming = crate::farming::FarmingSystem::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("farming content validation failed: {error}"));

        // Register map-authored farming plots from every chunk's `farmingPlots`.
        for coord in &chunk_coords {
            if let Some(chunk) = world.get_or_load_chunk(*coord).await {
                for plot in &chunk.farming_plots {
                    farming.register_patch(crate::farming::FarmingPatch {
                        id: plot.id.clone(),
                        x: plot.world_x,
                        y: plot.world_y,
                        patch_type: plot.patch_type.clone(),
                        plot: 1,
                        width: plot.width,
                        height: plot.height,
                        capacity: plot.capacity,
                    });
                }
            }
        }

        tracing::info!(
            "Loaded farming system with {} crops, {} patches",
            farming.crops.len(),
            farming.patches.len()
        );
        let mut resource_contracts = crate::resource_contracts::ResourceContractManager::new();

        // Restore planted patches from database. Rows still keyed to a legacy
        // hardcoded patch id (e.g. "p1_allotment_2") that no longer exists in the
        // map are retired: their seeds are refunded to the player's bank and the
        // dead rows deleted. Map-authored plots use "fp_*" ids, so they're never
        // mistaken for legacy — and we never touch an *unknown* unregistered id, so
        // a chunk that fails to load this boot can't cause us to delete live crops.
        if let Some(ref db) = db {
            match db.load_farming_patches().await {
                Ok(saved_patches) => {
                    let mut restored = 0usize;
                    let mut legacy_refunds: HashMap<i64, crate::db::LegacyFarmingRefund> =
                        HashMap::new();
                    for row in &saved_patches {
                        if farming.patches.contains_key(&row.patch_id) {
                            farming.restore_patch(
                                &row.patch_id,
                                &row.player_id,
                                &row.crop_id,
                                row.planted_at,
                                row.lives_remaining,
                                &row.health,
                                row.composted,
                                row.disease_cycle_marker,
                            );
                            restored += 1;
                            continue;
                        }
                        if !is_legacy_farming_patch_id(&row.patch_id) {
                            continue;
                        }
                        let Some(character_id) = row
                            .player_id
                            .strip_prefix("char_")
                            .and_then(|id| id.parse::<i64>().ok())
                        else {
                            tracing::warn!(
                                "Legacy farming patch {} has unparseable player_id {}, skipping",
                                row.patch_id,
                                row.player_id
                            );
                            continue;
                        };
                        let plan = legacy_refunds.entry(character_id).or_insert_with(|| {
                            crate::db::LegacyFarmingRefund {
                                character_id,
                                player_id: row.player_id.clone(),
                                seeds: Vec::new(),
                                patch_ids: Vec::new(),
                            }
                        });
                        plan.patch_ids.push(row.patch_id.clone());
                        // Refund what the player sank into the bed: one seed per
                        // plant, i.e. the patch capacity. Allotment beds hold 4;
                        // every other patch type holds 1.
                        match farming.crops.get(&row.crop_id) {
                            Some(crop) => {
                                let quantity = if crop.category == "allotment" { 4 } else { 1 };
                                if let Some(seed) =
                                    plan.seeds.iter_mut().find(|(id, _)| *id == crop.seed_item)
                                {
                                    seed.1 += quantity;
                                } else {
                                    plan.seeds.push((crop.seed_item.clone(), quantity));
                                }
                            }
                            None => tracing::warn!(
                                "Legacy farming patch {} references unknown crop {}; dropping row without refund",
                                row.patch_id,
                                row.crop_id
                            ),
                        }
                    }
                    if restored > 0 {
                        tracing::info!("Restored {} planted farming patches from database", restored);
                    }
                    if !legacy_refunds.is_empty() {
                        let plans: Vec<_> = legacy_refunds.into_values().collect();
                        let rows_retired: usize = plans.iter().map(|p| p.patch_ids.len()).sum();
                        let seeds_refunded: i32 =
                            plans.iter().flat_map(|p| p.seeds.iter().map(|(_, q)| *q)).sum();
                        match db.retire_legacy_farming_patches(&plans).await {
                            Ok(()) => tracing::info!(
                                "Retired {} legacy farming patch row(s) across {} character(s), refunding {} seed(s) to banks",
                                rows_retired,
                                plans.len(),
                                seeds_refunded
                            ),
                            Err(e) => tracing::error!(
                                "Failed to retire legacy farming patches: {}",
                                e
                            ),
                        }
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
        let slayer_registry = Arc::new(
            crate::slayer::SlayerRegistry::load(std::path::Path::new("data"))
                .unwrap_or_else(|error| panic!("slayer content validation failed: {error}")),
        );

        // Load chair config
        let chairs_source = std::fs::read_to_string("data/chairs.toml")
            .unwrap_or_else(|error| panic!("failed to read chairs.toml: {error}"));
        let chairs_config: ChairsConfig = toml::from_str(&chairs_source)
            .unwrap_or_else(|error| panic!("invalid chairs.toml: {error}"));
        let mut chair_gids: HashMap<u32, Direction> = HashMap::new();
        for entry in chairs_config.chairs {
            let direction = match entry.direction.as_str() {
                "down" => Direction::Down,
                "left" => Direction::Left,
                "up" => Direction::Up,
                "right" => Direction::Right,
                invalid => panic!("invalid chair direction '{invalid}' for gid {}", entry.gid),
            };
            if chair_gids.insert(entry.gid, direction).is_some() {
                panic!("duplicate chair gid {}", entry.gid);
            }
        }
        tracing::info!("Loaded {} chair GID definitions", chair_gids.len());

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
        scroll_spell_registry
            .load_from_file(scroll_spells_path)
            .unwrap_or_else(|error| panic!("scroll spell content validation failed: {error}"));

        // Load persistent ground spawn definitions and create initial ground items
        let mut ground_spawn_manager =
            crate::ground_spawn::GroundSpawnManager::load(std::path::Path::new("data"))
                .unwrap_or_else(|error| panic!("ground spawn content validation failed: {error}"));
        ground_spawn_manager
            .validate_items(&item_registry)
            .unwrap_or_else(|error| panic!("ground spawn reference validation failed: {error}"));
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
        let chest_spawns_source = std::fs::read_to_string("data/chest_spawns.toml")
            .unwrap_or_else(|error| panic!("failed to read chest_spawns.toml: {error}"));
        let overworld_chest_spawns =
            toml::from_str::<crate::chest::ChestSpawnsFile>(&chest_spawns_source)
                .unwrap_or_else(|error| panic!("invalid chest_spawns.toml: {error}"))
                .chests;

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

        let waystone_manager = crate::waystone::WaystoneManager::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("waystone content validation failed: {error}"));
        waystone_manager
            .validate_quests(&quest_registry)
            .await
            .unwrap_or_else(|error| panic!("waystone reference validation failed: {error}"));
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
        let pvp_source = std::fs::read_to_string("data/pvp_zones.toml")
            .unwrap_or_else(|error| panic!("failed to read pvp_zones.toml: {error}"));
        let pvp_config: PvpZoneConfig = toml::from_str(&pvp_source)
            .unwrap_or_else(|error| panic!("invalid pvp_zones.toml: {error}"));
        let pvp_zones: HashSet<(i32, i32)> = pvp_config
            .zones
            .iter()
            .map(|zone| (zone.chunk_x, zone.chunk_y))
            .collect();
        tracing::info!("Loaded {} PVP zone chunks", pvp_zones.len());

        let dig_site_manager = crate::dig_site::DigSiteManager::load(std::path::Path::new("data"))
            .unwrap_or_else(|error| panic!("dig site validation failed: {error}"));
        dig_site_manager
            .validate_references(&entity_registry, &quest_registry)
            .await
            .unwrap_or_else(|error| panic!("dig site reference validation failed: {error}"));

        let crafting_order_registry = crafting_orders::CraftingOrderRegistry::load("data")
            .unwrap_or_else(|error| panic!("crafting order content validation failed: {error}"));
        crafting_order_registry
            .validate_items(&item_registry)
            .unwrap_or_else(|error| panic!("crafting order reference validation failed: {error}"));

        let crate_loot_registry = crate_loot::CrateLootRegistry::load("data")
            .unwrap_or_else(|error| panic!("crate loot validation failed: {error}"));
        crate_loot_registry
            .validate_items(&item_registry)
            .unwrap_or_else(|error| panic!("crate loot reference validation failed: {error}"));

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
            transport: RoomTransport::new(256),
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
            dig_site_manager: RwLock::new(dig_site_manager),
            waystone_manager: RwLock::new(waystone_manager),
            chest_registry,
            chest_manager: RwLock::new(chest_manager),
            player_open_chests: RwLock::new(HashMap::new()),
            trades: RwLock::new(HashMap::new()),
            player_trades: RwLock::new(HashMap::new()),
            trade_requests: RwLock::new(HashMap::new()),
            overworld_world_map,
            pvp_zones,
            movement_anomalies: MovementAnomalyCounters::default(),
            crafting_order_registry,
            crate_loot_registry,
            top_level_player_name: RwLock::new(None),
            top_level_value: RwLock::new(0),
            second_level_player_name: RwLock::new(None),
            second_level_value: RwLock::new(0),
        }
    }
}

/// Legacy hardcoded farming patches used ids like "p1_allotment_2" (a leading
/// `p` followed by a digit). Map-authored plots use "fp_*" ids, so this never
/// matches a live plot — letting us safely retire stale planted rows.
fn is_legacy_farming_patch_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next() == Some('p') && chars.next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::is_legacy_farming_patch_id;

    #[test]
    fn legacy_ids_match_but_map_authored_ids_do_not() {
        assert!(is_legacy_farming_patch_id("p1_allotment_2"));
        assert!(is_legacy_farming_patch_id("p3_tree_1"));
        // Map-authored plots must never be treated as legacy/retirable.
        assert!(!is_legacy_farming_patch_id("fp_1781635030232_hdtbt9w"));
        assert!(!is_legacy_farming_patch_id("patch_1"));
        assert!(!is_legacy_farming_patch_id(""));
    }
}
