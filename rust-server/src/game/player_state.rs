use super::*;

impl GameRoom {
    pub async fn reserve_player(
        &self,
        player_id: &str,
        name: &str,
        gender: &str,
        skin: &str,
        hair_style: Option<i32>,
        hair_color: Option<i32>,
    ) {
        let (spawn_x, spawn_y) = self.world.get_spawn_position().await;
        let mut players = self.players.write().await;
        let player = Player::new(
            player_id, name, spawn_x, spawn_y, gender, skin, hair_style, hair_color,
        );
        players.insert(player_id.to_string(), player);

        // Track player's starting chunk
        let chunk = ChunkCoord::from_world(spawn_x, spawn_y);
        let mut chunks = self.player_chunks.write().await;
        chunks.insert(player_id.to_string(), chunk);
    }

    pub async fn reserve_player_with_data(&self, player_id: &str, data: PlayerRestoreData) {
        let PlayerRestoreData {
            name,
            x,
            y,
            z,
            hp,
            prayer_points,
            mp,
            skills,
            gold,
            inventory_json,
            gender,
            skin,
            hair_style,
            hair_color,
            equipped_head,
            equipped_body,
            equipped_weapon,
            equipped_back,
            equipped_feet,
            equipped_ring,
            equipped_gloves,
            equipped_necklace,
            equipped_belt,
            is_admin,
            account_id,
            ip_address,
            sitting_at_x,
            sitting_at_y,
            bank_json,
            bank_gold,
            bank_max_slots,
            combat_style_prefs_json,
        } = data;

        // Validate saved position — if the chunk doesn't exist on disk, reset to spawn
        let (safe_x, safe_y, safe_z) = {
            let coord = ChunkCoord::from_world(x, y);
            if self.world.chunk_file_exists(coord) {
                (x, y, z)
            } else {
                tracing::warn!(
                    "Player {} has invalid position ({}, {}) — chunk {:?} missing on disk, resetting to spawn",
                    player_id,
                    x,
                    y,
                    coord
                );
                (WORLD_SPAWN_X, WORLD_SPAWN_Y, 0)
            }
        };

        let mut player = Player::new(
            player_id, &name, safe_x, safe_y, &gender, &skin, hair_style, hair_color,
        );
        player.z = safe_z;
        player.bank_max_slots = bank_max_slots;
        player.bank = item::Bank::new_with_size(bank_max_slots as usize);

        // Restore saved stats
        player.skills = skills;
        player.hp = hp.min(player.max_hp()); // Cap HP at max (hitpoints level)
        player.prayer_points = prayer_points.min(player.max_prayer_points());
        player.mp = mp.min(player.max_mp());
        // If player disconnected while dead (hp=0), respawn them
        if player.hp <= 0 {
            player.hp = player.max_hp();
            player.x = WORLD_SPAWN_X;
            player.y = WORLD_SPAWN_Y;
            player.z = 0;
        }
        player.inventory.gold = gold;
        player.equipped_head = equipped_head;
        player.equipped_body = equipped_body;
        player.equipped_weapon = equipped_weapon;
        player.equipped_back = equipped_back;
        player.equipped_feet = equipped_feet;
        player.equipped_ring = equipped_ring;
        player.equipped_gloves = equipped_gloves;
        player.equipped_necklace = equipped_necklace;
        player.equipped_belt = equipped_belt;
        player.is_admin = is_admin;
        player.account_id = account_id;
        player.ip_address = ip_address;

        // Restore combat style preferences and set active style based on equipped weapon
        if let Ok(prefs) = serde_json::from_str::<HashMap<String, String>>(&combat_style_prefs_json)
        {
            for (weapon_key, style_str) in &prefs {
                if let Some(style) = CombatStyle::from_str(style_str) {
                    player.combat_style_prefs.insert(weapon_key.clone(), style);
                }
            }
        }
        // Determine weapon type from equipped weapon and restore preferred style
        let weapon_type = player
            .equipped_weapon
            .as_ref()
            .and_then(|wid| self.item_registry.get(wid))
            .and_then(|def| def.equipment.as_ref())
            .map(|eq| eq.weapon_type)
            .unwrap_or(WeaponType::Melee);
        let weapon_key = match weapon_type {
            WeaponType::Melee => "melee",
            WeaponType::Ranged => "ranged",
        };
        if let Some(&pref_style) = player.combat_style_prefs.get(weapon_key)
            && pref_style.is_valid_for(weapon_type)
        {
            player.combat_style = pref_style;
        }

        // Restore inventory from JSON - support both old (u8) and new (String) formats
        // Skip invalid slots (empty item_id or quantity <= 0) to prevent ghost items
        if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(&inventory_json) {
            // New format: (slot_idx, item_id, quantity)
            for (slot_idx, item_id, quantity) in slots {
                if slot_idx < player.inventory.slots.len() && !item_id.is_empty() && quantity > 0 {
                    player.inventory.slots[slot_idx] =
                        Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        } else if let Ok(slots) = serde_json::from_str::<Vec<(usize, u8, i32)>>(&inventory_json) {
            // Legacy format: (slot_idx, item_type_u8, quantity) - migrate to string IDs
            for (slot_idx, item_type_u8, quantity) in slots {
                if slot_idx < player.inventory.slots.len() && quantity > 0 {
                    let item_id = match item_type_u8 {
                        0 => "health_potion",
                        1 => "mana_potion",
                        3 => "slime_core",
                        4 => "iron_ore",
                        5 => "goblin_ear",
                        _ => continue, // Skip unknown items (2 was gold, handled separately)
                    }
                    .to_string();
                    player.inventory.slots[slot_idx] =
                        Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        }

        // Restore bank from JSON
        player.bank.gold = bank_gold;
        if let Ok(slots) = serde_json::from_str::<Vec<(usize, String, i32)>>(&bank_json) {
            for (slot_idx, item_id, quantity) in slots {
                if slot_idx < player.bank.slots.len() && !item_id.is_empty() && quantity > 0 {
                    player.bank.slots[slot_idx] = Some(item::InventorySlot::new(item_id, quantity));
                }
            }
        }

        // Restore sitting state and set direction from chair before inserting into players map
        if let (Some(sx), Some(sy)) = (sitting_at_x, sitting_at_y) {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(sx, sy)) {
                player.sitting_at = Some((sx, sy));
                player.direction = chair.direction;
                chair.occupied_by = Some(player_id.to_string());
            }
            // If chair no longer exists, don't restore sitting state
        }

        tracing::info!(
            "Restored player {} at ({}, {}) with {} HP, combat level {}, {} gold, appearance: {} {}",
            &name,
            x,
            y,
            hp,
            player.combat_level(),
            gold,
            &gender,
            &skin
        );

        let mut players = self.players.write().await;
        players.insert(player_id.to_string(), player);
        drop(players);

        // Track player's starting chunk for systems that reference chunk residency.
        let chunk = ChunkCoord::from_world(x, y);
        let mut chunks = self.player_chunks.write().await;
        chunks.insert(player_id.to_string(), chunk);
    }

    pub async fn activate_player(&self, player_id: &str) -> String {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.active = true;
            // Initialize activity time so auto-retaliate works from login
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            player.last_activity_time = now;
            return player.name.clone();
        }
        "Unknown".to_string()
    }

    pub async fn remove_player(&self, player_id: &str) {
        // Handle arena disconnect
        {
            let mut arena = self.arena_manager.write().await;
            if let Some((disconnected_id, _killer_id)) = arena.on_player_disconnect(player_id) {
                // If was fighting and match should end, handle it
                if arena.is_fighting() && arena.check_match_end() {
                    let current_time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    let placements = arena.end_match(current_time);
                    drop(arena);

                    // Distribute rewards
                    {
                        let mut players = self.players.write().await;
                        for placement in &placements {
                            if placement.gold_reward > 0
                                && let Some(p) = players.get_mut(&placement.player_id)
                            {
                                p.inventory.gold = item::checked_gold_credit(
                                    p.inventory.gold,
                                    placement.gold_reward,
                                )
                                .unwrap_or(item::MAX_GOLD);
                            }
                        }
                    }

                    let placement_data: Vec<crate::protocol::ArenaPlacementData> = placements
                        .iter()
                        .map(|p| crate::protocol::ArenaPlacementData {
                            rank: p.rank,
                            player_id: p.player_id.clone(),
                            player_name: p.player_name.clone(),
                            kills: p.kills,
                            gold_reward: p.gold_reward,
                        })
                        .collect();

                    self.broadcast_to_arena(ServerMessage::ArenaMatchEnd {
                        placements: placement_data,
                    })
                    .await;

                    // Broadcast elimination for the disconnected player
                    self.broadcast_to_arena(ServerMessage::ArenaPlayerEliminated {
                        player_id: disconnected_id.clone(),
                        player_name: "Disconnected".to_string(),
                        killer_id: "disconnect".to_string(),
                        killer_name: "Disconnect".to_string(),
                        remaining: 0,
                    })
                    .await;
                } else {
                    // Refund if was queued (escrow removed in on_player_disconnect)
                    let _ = &disconnected_id; // already handled
                }
            }
        }

        // Free any chair the player was sitting on
        // Extract sitting position first, then release players lock before acquiring chairs lock
        let sitting_pos = {
            let players = self.players.read().await;
            players.get(player_id).and_then(|p| p.sitting_at)
        };
        if let Some((tx, ty)) = sitting_pos {
            let mut chairs = self.chairs.write().await;
            if let Some(chair) = chairs.get_mut(&(tx, ty))
                && chair.occupied_by.as_deref() == Some(player_id)
            {
                chair.occupied_by = None;
            }
        }

        // Close any open chest
        self.close_player_chest(player_id).await;

        // Cancel any active trade on disconnect
        self.cancel_trade_for_player(player_id, "Partner disconnected")
            .await;

        // Close stall on disconnect, return items to inventory/bank
        self.force_close_stall(player_id).await;

        // Stop any active gathering/woodcutting
        {
            let mut gathering = self.gathering.write().await;
            gathering.stop_gathering(player_id);
        }
        {
            let mut woodcutting = self.woodcutting.write().await;
            woodcutting.stop_woodcutting(player_id);
        }

        // Clean up player chunk tracking
        {
            let mut chunks = self.player_chunks.write().await;
            chunks.remove(player_id);
        }
        self.visible_ground_items.write().await.remove(player_id);

        // Clean up player quest states
        {
            let mut quest_states = self.player_quest_states.write().await;
            quest_states.remove(player_id);
        }
        self.npc_interaction_grants.write().await.remove(player_id);
        self.dialogue_grants.write().await.remove(player_id);

        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.auto_action = None;
        }
        players.remove(player_id);
    }

    pub async fn get_player_save_data(&self, player_id: &str) -> Option<PlayerSaveData> {
        // Check if player is in an instance and get the map_id
        let current_map = if self.player_instances.read().await.contains_key(player_id) {
            self.instance_manager
                .find_player_instance(player_id)
                .await
                .map(|inst| inst.map_id.clone())
        } else {
            None
        };

        let players = self.players.read().await;
        players.get(player_id).map(|p| {
            // Serialize inventory to JSON - new format with string item IDs
            // Filter out empty/invalid slots to prevent ghost items
            let inventory_slots: Vec<(usize, String, i32)> = p
                .inventory
                .slots
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    slot.as_ref()
                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                })
                .collect();
            let inventory_json =
                serde_json::to_string(&inventory_slots).unwrap_or_else(|_| "[]".to_string());

            // Serialize bank to JSON
            let bank_slots: Vec<(usize, String, i32)> = p
                .bank
                .slots
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    slot.as_ref()
                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                })
                .collect();
            let bank_json = serde_json::to_string(&bank_slots).unwrap_or_else(|_| "[]".to_string());

            PlayerSaveData {
                x: p.x as f32,
                y: p.y as f32,
                z: p.z,
                hp: p.hp,
                prayer_points: p.prayer_points,
                mp: p.mp,
                skills: p.skills.clone(),
                gold: p.inventory.gold,
                inventory_json,
                gender: p.gender.clone(),
                skin: p.skin.clone(),
                equipped_head: p.equipped_head.clone(),
                equipped_body: p.equipped_body.clone(),
                equipped_weapon: p.equipped_weapon.clone(),
                equipped_back: p.equipped_back.clone(),
                equipped_feet: p.equipped_feet.clone(),
                equipped_ring: p.equipped_ring.clone(),
                equipped_gloves: p.equipped_gloves.clone(),
                equipped_necklace: p.equipped_necklace.clone(),
                equipped_belt: p.equipped_belt.clone(),
                current_map: current_map.clone(),
                sitting_at_x: p.sitting_at.map(|(x, _)| x),
                sitting_at_y: p.sitting_at.map(|(_, y)| y),
                entrance_x: None, // Filled in by caller from player_entrance_positions
                entrance_y: None,
                bank_json,
                bank_gold: p.bank.gold,
                bank_max_slots: p.bank_max_slots,
                combat_style_prefs: {
                    let prefs_map: HashMap<&str, &str> = p
                        .combat_style_prefs
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    serde_json::to_string(&prefs_map).unwrap_or_else(|_| "{}".to_string())
                },
            }
        })
    }

    pub async fn get_bulk_save_data(
        &self,
        player_ids: &[String],
    ) -> HashMap<
        String,
        (
            PlayerSaveData,
            Option<PlayerQuestState>,
            HashSet<String>,
            Option<crate::slayer::PlayerSlayerState>,
            HashSet<String>,
        ),
    > {
        struct RawPlayerSnapshot {
            x: i32,
            y: i32,
            z: i32,
            hp: i32,
            prayer_points: i32,
            mp: i32,
            skills: Skills,
            gold: i32,
            inventory_slots: Vec<(usize, String, i32)>,
            gender: String,
            skin: String,
            equipped_head: Option<String>,
            equipped_body: Option<String>,
            equipped_weapon: Option<String>,
            equipped_back: Option<String>,
            equipped_feet: Option<String>,
            equipped_ring: Option<String>,
            equipped_gloves: Option<String>,
            equipped_necklace: Option<String>,
            equipped_belt: Option<String>,
            sitting_at_x: Option<i32>,
            sitting_at_y: Option<i32>,
            recipes: HashSet<String>,
            unlocked_spells: HashSet<String>,
            bank_slots: Vec<(usize, String, i32)>,
            bank_gold: i32,
            bank_max_slots: u32,
            combat_style_prefs: HashMap<String, CombatStyle>,
        }

        let mut result = HashMap::new();

        // Snapshot instance assignments once
        let instance_map: HashMap<String, String> = {
            let instances = self.player_instances.read().await;
            player_ids
                .iter()
                .filter_map(|pid| instances.get(pid).map(|inst| (pid.clone(), inst.clone())))
                .collect()
        };

        // Resolve map_ids for players in instances (batch)
        let mut map_ids: HashMap<String, String> = HashMap::new();
        for pid in instance_map.keys() {
            if let Some(inst) = self.instance_manager.find_player_instance(pid).await {
                map_ids.insert(pid.clone(), inst.map_id.clone());
            }
        }

        // Single lock on players to snapshot all mutable gameplay state.
        // Keep this lock scope minimal; expensive JSON serialization happens after unlock.
        let raw_snapshots: HashMap<String, RawPlayerSnapshot> = {
            let players = self.players.read().await;
            let mut snapshots = HashMap::new();
            for pid in player_ids {
                if let Some(p) = players.get(pid) {
                    let inventory_slots: Vec<(usize, String, i32)> = p
                        .inventory
                        .slots
                        .iter()
                        .enumerate()
                        .filter_map(|(idx, slot)| {
                            slot.as_ref()
                                .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                                .map(|s| (idx, s.item_id.clone(), s.quantity))
                        })
                        .collect();

                    snapshots.insert(
                        pid.clone(),
                        RawPlayerSnapshot {
                            x: p.x,
                            y: p.y,
                            z: p.z,
                            hp: p.hp,
                            prayer_points: p.prayer_points,
                            mp: p.mp,
                            skills: p.skills.clone(),
                            gold: p.inventory.gold,
                            inventory_slots,
                            gender: p.gender.clone(),
                            skin: p.skin.clone(),
                            equipped_head: p.equipped_head.clone(),
                            equipped_body: p.equipped_body.clone(),
                            equipped_weapon: p.equipped_weapon.clone(),
                            equipped_back: p.equipped_back.clone(),
                            equipped_feet: p.equipped_feet.clone(),
                            equipped_ring: p.equipped_ring.clone(),
                            equipped_gloves: p.equipped_gloves.clone(),
                            equipped_necklace: p.equipped_necklace.clone(),
                            equipped_belt: p.equipped_belt.clone(),
                            sitting_at_x: p.sitting_at.map(|(x, _)| x),
                            sitting_at_y: p.sitting_at.map(|(_, y)| y),
                            recipes: p.discovered_recipes.clone(),
                            unlocked_spells: p.unlocked_spells.clone(),
                            bank_slots: p
                                .bank
                                .slots
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, slot)| {
                                    slot.as_ref()
                                        .filter(|s| s.quantity > 0 && !s.item_id.is_empty())
                                        .map(|s| (idx, s.item_id.clone(), s.quantity))
                                })
                                .collect(),
                            bank_gold: p.bank.gold,
                            bank_max_slots: p.bank_max_slots,
                            combat_style_prefs: p.combat_style_prefs.clone(),
                        },
                    );
                }
            }
            snapshots
        };

        // Build save payloads outside the players lock.
        for (pid, raw) in raw_snapshots {
            let inventory_json =
                serde_json::to_string(&raw.inventory_slots).unwrap_or_else(|_| "[]".to_string());
            let save_data = PlayerSaveData {
                x: raw.x as f32,
                y: raw.y as f32,
                z: raw.z,
                hp: raw.hp,
                prayer_points: raw.prayer_points,
                mp: raw.mp,
                skills: raw.skills,
                gold: raw.gold,
                inventory_json,
                gender: raw.gender,
                skin: raw.skin,
                equipped_head: raw.equipped_head,
                equipped_body: raw.equipped_body,
                equipped_weapon: raw.equipped_weapon,
                equipped_back: raw.equipped_back,
                equipped_feet: raw.equipped_feet,
                equipped_ring: raw.equipped_ring,
                equipped_gloves: raw.equipped_gloves,
                equipped_necklace: raw.equipped_necklace,
                equipped_belt: raw.equipped_belt,
                current_map: map_ids.get(&pid).cloned(),
                sitting_at_x: raw.sitting_at_x,
                sitting_at_y: raw.sitting_at_y,
                entrance_x: None, // Filled in by caller from player_entrance_positions
                entrance_y: None,
                bank_json: serde_json::to_string(&raw.bank_slots)
                    .unwrap_or_else(|_| "[]".to_string()),
                bank_gold: raw.bank_gold,
                bank_max_slots: raw.bank_max_slots,
                combat_style_prefs: {
                    let prefs_map: HashMap<&str, &str> = raw
                        .combat_style_prefs
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    serde_json::to_string(&prefs_map).unwrap_or_else(|_| "{}".to_string())
                },
            };
            result.insert(
                pid,
                (save_data, None, raw.recipes, None, raw.unlocked_spells),
            );
        }

        // Single lock on quest states
        {
            let quest_states = self.player_quest_states.read().await;
            for pid in player_ids {
                if let Some(entry) = result.get_mut(pid) {
                    entry.1 = quest_states.get(pid).cloned();
                }
            }
        }

        // Single lock on slayer states
        {
            let slayer_states = self.player_slayer_states.read().await;
            for pid in player_ids {
                if let Some(entry) = result.get_mut(pid) {
                    entry.3 = slayer_states.get(pid).cloned();
                }
            }
        }

        result
    }

    pub async fn set_player_quest_state(&self, player_id: &str, state: PlayerQuestState) {
        let mut quest_states = self.player_quest_states.write().await;
        quest_states.insert(player_id.to_string(), state);
    }

    pub async fn get_player_quest_state(&self, player_id: &str) -> Option<PlayerQuestState> {
        let quest_states = self.player_quest_states.read().await;
        quest_states.get(player_id).cloned()
    }

    pub async fn set_player_slayer_state(
        &self,
        player_id: &str,
        state: crate::slayer::PlayerSlayerState,
    ) {
        self.player_slayer_states
            .write()
            .await
            .insert(player_id.to_string(), state);
    }

    pub async fn get_player_slayer_state(
        &self,
        player_id: &str,
    ) -> crate::slayer::PlayerSlayerState {
        let mut state = self
            .player_slayer_states
            .read()
            .await
            .get(player_id)
            .cloned()
            .unwrap_or_default();
        // Migration: fix old "living_rock" task IDs -> "rock"
        if let Some(ref mut task) = state.current_task
            && task.monster_id == "living_rock"
        {
            task.monster_id = "rock".to_string();
            // Persist the fix
            self.player_slayer_states
                .write()
                .await
                .insert(player_id.to_string(), state.clone());
        }
        state
    }

    pub async fn set_player_discovered_recipes(&self, player_id: &str, recipes: HashSet<String>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.discovered_recipes = recipes;
        }
    }

    pub async fn get_player_discovered_recipes(&self, player_id: &str) -> HashSet<String> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.discovered_recipes.clone())
            .unwrap_or_default()
    }

    pub async fn discover_recipe(&self, player_id: &str, recipe_id: &str) -> bool {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.discovered_recipes.insert(recipe_id.to_string())
        } else {
            false
        }
    }

    pub async fn record_collection_entry(
        &self,
        player_id: &str,
        item_id: &str,
        source: &str,
        source_detail: &str,
    ) -> bool {
        let key = (item_id.to_string(), source.to_string());

        // Check + insert in-memory
        let is_new = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.collection_log.insert(key)
            } else {
                false
            }
        };

        if !is_new {
            return false;
        }

        // Persist to DB using character_id (parsed from player_id "char_123")
        let obtained_at = chrono::Utc::now().to_rfc3339();
        if let Some(ref db) = self.db
            && let Some(character_id) = Self::parse_character_id(player_id)
            && let Err(e) = db
                .save_collection_entry(character_id, item_id, source, source_detail, &obtained_at)
                .await
        {
            tracing::warn!("Failed to save collection entry for {}: {}", player_id, e);
        }

        // Send real-time notification to client
        self.send_to_player(
            player_id,
            crate::protocol::ServerMessage::CollectionLogEntry {
                item_id: item_id.to_string(),
                source: source.to_string(),
                source_detail: source_detail.to_string(),
                obtained_at,
            },
        )
        .await;

        true
    }

    pub async fn get_player_collection_log(&self, player_id: &str) -> HashSet<(String, String)> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.collection_log.clone())
            .unwrap_or_default()
    }

    pub async fn set_player_collection_log(&self, player_id: &str, log: HashSet<(String, String)>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.collection_log = log;
        }
    }

    pub async fn set_player_active_title(&self, player_id: &str, title: Option<String>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.active_title = title;
        }
    }

    pub async fn set_player_unlocked_spells(&self, player_id: &str, spells: HashSet<String>) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.unlocked_spells = spells;
        }
    }

    pub async fn get_player_unlocked_spells(&self, player_id: &str) -> HashSet<String> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| p.unlocked_spells.clone())
            .unwrap_or_default()
    }

    pub fn get_scroll_spell_definitions_message(&self) -> ServerMessage {
        let spells: Vec<crate::protocol::ScrollSpellDefData> = self
            .scroll_spell_registry
            .all()
            .iter()
            .map(|(id, def)| crate::protocol::ScrollSpellDefData {
                id: id.clone(),
                name: def.name.clone(),
                spell_type: match def.spell_type {
                    crate::spell::SpellType::Damage => "damage".to_string(),
                    crate::spell::SpellType::Heal => "heal".to_string(),
                    crate::spell::SpellType::Teleport => "teleport".to_string(),
                },
                mana_cost: def.mana_cost,
                cooldown_ms: def.cooldown_ms,
                base_power: def.base_power,
                effect_sprite: def.effect_sprite.clone(),
                pushback_distance: def.pushback_distance,
                wall_slam_damage_per_tile: def.wall_slam_damage_per_tile,
                description: def.description.clone(),
            })
            .collect();
        ServerMessage::ScrollSpellDefinitions { spells }
    }

    pub async fn get_active_quest_messages(&self, player_id: &str) -> Vec<ServerMessage> {
        let quest_states = self.player_quest_states.read().await;
        let quest_state = match quest_states.get(player_id) {
            Some(state) => state,
            None => return Vec::new(),
        };

        let mut messages = Vec::new();
        for (quest_id, progress) in &quest_state.active_quests {
            if let Some(quest) = self.quest_registry.get(quest_id).await {
                let objectives: Vec<QuestObjectiveData> = quest
                    .objectives
                    .iter()
                    .map(|o| {
                        // Get current progress from saved state
                        let (current, completed) = progress
                            .objectives
                            .get(&o.id)
                            .map(|p| (p.current, p.completed))
                            .unwrap_or((0, false));
                        QuestObjectiveData {
                            id: o.id.clone(),
                            description: o.description.clone(),
                            current,
                            target: o.count,
                            completed,
                        }
                    })
                    .collect();
                messages.push(ServerMessage::QuestAccepted {
                    quest_id: quest_id.clone(),
                    quest_name: quest.name.clone(),
                    objectives,
                });
            }
        }
        messages
    }

    pub async fn get_completed_quest_sync_message(&self, player_id: &str) -> ServerMessage {
        let quest_states = self.player_quest_states.read().await;
        let completed_quest_ids = quest_states
            .get(player_id)
            .map(|state| state.completed_quests.clone())
            .unwrap_or_default();

        ServerMessage::QuestStateSync {
            completed_quest_ids,
        }
    }

    pub async fn build_quest_catalog(&self) -> ServerMessage {
        let all_quests = self.quest_registry.all_quests().await;
        let npcs = self.npcs.read().await;

        // Build a map of prototype_id -> display_name from loaded NPCs
        let npc_names: std::collections::HashMap<String, String> = npcs
            .values()
            .map(|npc| (npc.prototype_id.clone(), npc.stats.display_name.clone()))
            .collect();

        let mut entries: Vec<QuestCatalogEntryData> = Vec::new();
        for quest in &all_quests {
            let giver_npc_name = npc_names
                .get(&quest.giver_npc)
                .cloned()
                .unwrap_or_else(|| quest.giver_npc.clone());

            // Resolve prerequisite quest name
            let (required_quest_id, required_quest_name) =
                if let Some(ref prev_id) = quest.chain.previous {
                    let prev_name = all_quests
                        .iter()
                        .find(|q| q.id == *prev_id)
                        .map(|q| q.name.clone());
                    (Some(prev_id.clone()), prev_name)
                } else {
                    (None, None)
                };

            let objectives = quest
                .objectives
                .iter()
                .map(|o| QuestObjectiveData {
                    id: o.id.clone(),
                    description: o.description.clone(),
                    current: 0,
                    target: o.count,
                    completed: false,
                })
                .collect();
            entries.push(QuestCatalogEntryData {
                quest_id: quest.id.clone(),
                name: quest.name.clone(),
                description: quest.description.clone(),
                giver_npc_name,
                level_required: quest.level_required,
                required_quest_id,
                required_quest_name,
                objectives,
            });
        }

        ServerMessage::QuestCatalog { quests: entries }
    }

    pub async fn player_count(&self) -> usize {
        let players = self.players.read().await;
        players.values().filter(|p| p.active).count()
    }

    pub async fn get_all_players(&self) -> Vec<Player> {
        let players = self.players.read().await;
        players.values().filter(|p| p.active).cloned().collect()
    }

    pub async fn get_visible_players(&self, player_id: &str) -> Vec<Player> {
        let players = self.players.read().await;
        let Some(source) = players.get(player_id) else {
            return Vec::new();
        };
        let instances = self.player_instances.read().await;
        let source_instance = instances.get(player_id).map(String::as_str);
        players
            .values()
            .filter(|player| {
                player.id != player_id
                    && player.active
                    && is_visible_event_recipient(
                        source_instance,
                        source.x,
                        source.y,
                        instances.get(&player.id).map(String::as_str),
                        player.x,
                        player.y,
                    )
            })
            .cloned()
            .collect()
    }

    pub async fn get_player_sitting_info(&self, player_id: &str) -> Option<(i32, i32, u8)> {
        let sitting_at = {
            let players = self.players.read().await;
            players.get(player_id)?.sitting_at?
        };
        let (sx, sy) = sitting_at;
        let chairs = self.chairs.read().await;
        let chair = chairs.get(&(sx, sy))?;
        Some((sx, sy, chair.direction as u8))
    }

    pub async fn get_player_position(&self, player_id: &str) -> Option<(i32, i32)> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| (p.x, p.y))
    }

    pub async fn set_player_position(&self, player_id: &str, x: i32, y: i32) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.x = x;
            player.y = y;
        }
    }

    pub async fn set_player_position_and_z(&self, player_id: &str, x: i32, y: i32, z: i32) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            player.x = x;
            player.y = y;
            player.z = z;
            player.grounded = true;
        }
    }

    pub async fn set_combat_style(&self, player_id: &str, style: CombatStyle) {
        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            // Get current weapon type to validate style
            let weapon_type = player
                .equipped_weapon
                .as_ref()
                .and_then(|wid| self.item_registry.get(wid))
                .and_then(|def| def.equipment.as_ref())
                .map(|eq| eq.weapon_type)
                .unwrap_or(WeaponType::Melee);

            // Only set if style is valid for current weapon type
            if style.is_valid_for(weapon_type) {
                player.combat_style = style;
                // Save preference for this weapon type
                let weapon_key = match weapon_type {
                    WeaponType::Melee => "melee",
                    WeaponType::Ranged => "ranged",
                };
                player
                    .combat_style_prefs
                    .insert(weapon_key.to_string(), style);
            }
        }
    }

    pub async fn get_player_appearance(&self, player_id: &str) -> Option<(String, String)> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| (p.gender.clone(), p.skin.clone()))
    }

    pub async fn get_player_hair(&self, player_id: &str) -> Option<(Option<i32>, Option<i32>)> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| (p.hair_style, p.hair_color))
    }

    pub async fn get_player_name(&self, player_id: &str) -> Option<String> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| p.name.clone())
    }

    pub async fn get_ground_items_in_instance(
        &self,
        instance_id: Option<&str>,
    ) -> Vec<ServerMessage> {
        let items = self.ground_items.read().await;
        items
            .values()
            .filter(|item| {
                match (&item.instance_id, instance_id) {
                    (None, None) => true,         // Both overworld
                    (Some(a), Some(b)) => a == b, // Same instance
                    _ => false,                   // Different zones
                }
            })
            .map(|item| ServerMessage::ItemDropped {
                id: item.id.clone(),
                item_id: item.item_id.clone(),
                x: item.x,
                y: item.y,
                quantity: item.quantity,
            })
            .collect()
    }

    /// Get ground items currently visible to a player.
    pub async fn get_visible_ground_items(&self, player_id: &str) -> Vec<ServerMessage> {
        let (player_x, player_y) = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return Vec::new();
            };
            (player.x, player.y)
        };
        let instance_id = self.player_instances.read().await.get(player_id).cloned();
        let items = self.ground_items.read().await;
        let visible_items: Vec<&GroundItem> = items
            .values()
            .filter(|item| {
                let same_instance = match (&item.instance_id, instance_id.as_ref()) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                same_instance
                    && is_within_view(
                        player_x,
                        player_y,
                        item.x.floor() as i32,
                        item.y.floor() as i32,
                    )
            })
            .collect();
        self.visible_ground_items.write().await.insert(
            player_id.to_string(),
            visible_items.iter().map(|item| item.id.clone()).collect(),
        );
        visible_items
            .into_iter()
            .map(|item| ServerMessage::ItemDropped {
                id: item.id.clone(),
                item_id: item.item_id.clone(),
                x: item.x,
                y: item.y,
                quantity: item.quantity,
            })
            .collect()
    }

    pub(super) async fn sync_ground_item_visibility(&self) {
        let senders = self.transport.player_senders().await;
        let players = self.players.read().await;
        let instances = self.player_instances.read().await;
        let items = self.ground_items.read().await;
        let mut visibility = self.visible_ground_items.write().await;

        for (player_id, sender) in senders.iter() {
            let Some(player) = players.get(player_id) else {
                continue;
            };
            let player_instance = instances.get(player_id);
            let current: HashSet<String> = items
                .values()
                .filter(|item| {
                    is_visible_event_recipient(
                        item.instance_id.as_deref(),
                        item.x.floor() as i32,
                        item.y.floor() as i32,
                        player_instance.map(String::as_str),
                        player.x,
                        player.y,
                    )
                })
                .map(|item| item.id.clone())
                .collect();
            let known = visibility.entry(player_id.clone()).or_default();

            for item_id in current.difference(known) {
                if let Some(item) = items.get(item_id) {
                    let msg = ServerMessage::ItemDropped {
                        id: item.id.clone(),
                        item_id: item.item_id.clone(),
                        x: item.x,
                        y: item.y,
                        quantity: item.quantity,
                    };
                    if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
                        let _ = sender.try_send(bytes);
                    }
                }
            }
            for item_id in known.difference(&current) {
                let msg = ServerMessage::ItemDespawned {
                    item_id: item_id.clone(),
                };
                if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
                    let _ = sender.try_send(bytes);
                }
            }
            *known = current;
        }
    }

    pub async fn get_player_inventory_update(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .map(|p| ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: p.inventory.to_update(),
                gold: p.inventory.gold,
            })
    }

    pub async fn get_player_skills_sync(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| ServerMessage::SkillsSync {
            player_id: player_id.to_string(),
            hitpoints_level: p.skills.hitpoints.level,
            hitpoints_xp: p.skills.hitpoints.xp,
            attack_level: p.skills.attack.level,
            attack_xp: p.skills.attack.xp,
            strength_level: p.skills.strength.level,
            strength_xp: p.skills.strength.xp,
            defence_level: p.skills.defence.level,
            defence_xp: p.skills.defence.xp,
            ranged_level: p.skills.ranged.level,
            ranged_xp: p.skills.ranged.xp,
            fishing_level: p.skills.fishing.level,
            fishing_xp: p.skills.fishing.xp,
            farming_level: p.skills.farming.level,
            farming_xp: p.skills.farming.xp,
            smithing_level: p.skills.smithing.level,
            smithing_xp: p.skills.smithing.xp,
            prayer_level: p.skills.prayer.level,
            prayer_xp: p.skills.prayer.xp,
            magic_level: p.skills.magic.level,
            magic_xp: p.skills.magic.xp,
            woodcutting_level: p.skills.woodcutting.level,
            woodcutting_xp: p.skills.woodcutting.xp,
            alchemy_level: p.skills.alchemy.level,
            alchemy_xp: p.skills.alchemy.xp,
            mining_level: p.skills.mining.level,
            mining_xp: p.skills.mining.xp,
            slayer_level: p.skills.slayer.level,
            slayer_xp: p.skills.slayer.xp,
            survivalist_level: p.skills.survivalist.level,
            survivalist_xp: p.skills.survivalist.xp,
        })
    }

    pub(super) fn build_potion_buffs_sync(player_id: &str, player: &Player) -> ServerMessage {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        ServerMessage::PotionBuffsSync {
            player_id: player_id.to_string(),
            buffs: player
                .active_buffs
                .iter()
                .map(|b| crate::protocol::PotionBuffEntry {
                    stat: b.stat.clone(),
                    amount: b.amount,
                    remaining_ms: b.expires_at.saturating_sub(now),
                    source_item_id: b.source_item_id.clone(),
                })
                .collect(),
        }
    }

    pub async fn get_player_potion_buffs_sync(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players
            .get(player_id)
            .filter(|p| !p.active_buffs.is_empty())
            .map(|p| Self::build_potion_buffs_sync(player_id, p))
    }

    pub async fn get_all_npcs(&self) -> Vec<Npc> {
        let npcs = self.npcs.read().await;
        npcs.values().cloned().collect()
    }

    pub async fn spawn_npc_at(&self, prototype_id: &str, x: f32, y: f32) -> Option<String> {
        let Some(prototype) = self.entity_registry.get(prototype_id) else {
            tracing::warn!("Cannot spawn NPC: prototype '{}' not found", prototype_id);
            return None;
        };

        let npc_id = format!("admin_npc_{}", Uuid::new_v4());
        let npc = Npc::from_prototype(
            &npc_id,
            prototype_id,
            prototype,
            x as i32,
            y as i32,
            1, // Default level
            None,
        );

        let mut npcs = self.npcs.write().await;
        npcs.insert(npc_id.clone(), npc);
        tracing::info!("Admin spawned NPC {} at ({}, {})", prototype_id, x, y);
        Some(npc_id)
    }
}
