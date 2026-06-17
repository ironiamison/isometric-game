use super::*;

type SenderRef<'a> = (&'a String, &'a mpsc::Sender<Vec<u8>>);

impl GameRoom {
    pub async fn tick(&self) -> TickTelemetry {
        let mut tick_telemetry = TickTelemetry {
            movement_stale_packets_ignored: self
                .movement_anomalies
                .stale_packets_ignored
                .swap(0, Ordering::Relaxed) as usize,
            movement_seq_gap_events: self
                .movement_anomalies
                .seq_gap_events
                .swap(0, Ordering::Relaxed) as usize,
            movement_input_gap_events: self
                .movement_anomalies
                .input_gap_events
                .swap(0, Ordering::Relaxed) as usize,
            ..TickTelemetry::default()
        };
        let tick_start = std::time::Instant::now();
        let mut chunk_unload_ms = 0u128;
        let mut restock_ms = 0u128;
        let delta_time = 1.0 / TICK_RATE;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Update tick counter and get current tick for movement timing
        let current_tick = {
            let mut tick = self.tick.write().await;
            *tick += 1;
            *tick
        };

        // Periodically unload distant chunks to prevent unbounded memory/CPU growth
        if current_tick % 100 == 0 {
            let unload_start = std::time::Instant::now();

            // Use live player positions as the source of truth so stale chunk tracking
            // cannot unload chunks around actively moving players.
            let instanced_players: HashSet<String> = {
                let instances = self.player_instances.read().await;
                instances.keys().cloned().collect()
            };

            let active_coords: Vec<ChunkCoord> = {
                let players = self.players.read().await;
                players
                    .values()
                    .filter(|p| p.active && p.is_alive() && !instanced_players.contains(&p.id))
                    .map(|p| ChunkCoord::from_world(p.x, p.y))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect()
            };

            if !active_coords.is_empty() {
                self.world.unload_distant_chunks(&active_coords, 5).await;
            }
            chunk_unload_ms = unload_start.elapsed().as_millis();
        }

        let pre_npc_start = std::time::Instant::now();

        self.handle_player_respawns(current_time).await;

        let movement_state = self
            .process_player_movement_tick(current_time, current_tick, &mut tick_telemetry)
            .await;
        let gathering_player_ids = movement_state.gathering_player_ids;
        let moved_players = movement_state.moved_players;
        let woodcutting_player_ids = movement_state.woodcutting_player_ids;
        let woodcutting_stopped = movement_state.woodcutting_stopped;

        let prayer_drain_ms = self.process_player_resource_ticks(current_tick).await;

        let player_updates = self
            .collect_player_updates(&gathering_player_ids, &woodcutting_player_ids)
            .await;

        self.handle_post_movement_effects(&moved_players, woodcutting_stopped)
            .await;

        let pre_npc_ms = pre_npc_start.elapsed().as_millis();
        let npc_world_start = std::time::Instant::now();

        let overworld_visibility = self.collect_overworld_visibility_snapshot().await;
        let player_positions = overworld_visibility.player_positions;
        let players_by_chunk = overworld_visibility.players_by_chunk;

        let overworld_npc_tick = self
            .process_overworld_npc_tick(
                current_time,
                delta_time,
                &player_positions,
                &players_by_chunk,
            )
            .await;
        let npc_updates = overworld_npc_tick.npc_updates;
        let respawned_npcs = overworld_npc_tick.respawned_npcs;
        let mut npc_attacks = overworld_npc_tick.npc_attacks;
        self.send_npc_speech_events(overworld_npc_tick.npc_speech_events)
            .await;

        let instance_npc_tick = self
            .process_instance_npc_tick(current_time, delta_time)
            .await;
        npc_attacks.extend(instance_npc_tick.npc_attacks);
        self.send_npc_speech_events(instance_npc_tick.speech_events)
            .await;

        // Process explosive minion contact explosions
        for (npc_id, instance_id, npc_x, npc_y) in instance_npc_tick.minion_explosions {
            self.check_boss_minion_death(&npc_id, &instance_id, npc_x, npc_y, current_time)
                .await;
            self.check_pharaoh_minion_death(&npc_id, &instance_id, current_time)
                .await;
        }

        // Process NPC attacks on players using hit/miss mechanics
        for (npc_id, target_id, npc_level, max_hit, npc_attack_bonus) in npc_attacks {
            // Players in gathering zones are immune to NPC damage
            {
                let gathering = self.gathering.read().await;
                if gathering.is_gathering(&target_id) {
                    continue;
                }
            }
            let (target_hp, target_x, target_y, died, damage): (i32, f32, f32, bool, i32) = {
                let mut players = self.players.write().await;
                if let Some(target) = players.get_mut(&target_id) {
                    if target.is_dead {
                        continue; // Already dead
                    }
                    // God mode prevents all damage
                    if target.is_god_mode {
                        continue;
                    }

                    // NPC uses its level as attack level
                    let npc_attack_level = npc_level;

                    // Player uses their defence skill level and equipment bonus
                    let player_defence_level = target.skills.defence.level;
                    let base_defence_bonus = target.defence_bonus(&self.item_registry);

                    // Apply prayer bonuses to player's defence
                    let active_ids: Vec<String> = target.active_prayers.iter().cloned().collect();
                    let prayer_effects = self.prayer_registry.calculate_effects(&active_ids);
                    let player_defence_bonus =
                        prayer_effects.apply_defence_bonus(base_defence_bonus);

                    // Roll hit/miss
                    if !calculate_hit(
                        npc_attack_level,
                        npc_attack_bonus,
                        player_defence_level,
                        player_defence_bonus,
                    ) {
                        // Miss - deal 0 damage
                        tracing::debug!(
                            "NPC {} misses {} (atk {} vs def {} + {})",
                            npc_id,
                            target_id,
                            npc_attack_level,
                            player_defence_level,
                            player_defence_bonus
                        );
                        (target.hp, target.x as f32, target.y as f32, false, 0)
                    } else {
                        // Hit - roll damage and apply with prayer damage reduction
                        let raw_damage = roll_damage(max_hit);
                        let damage = prayer_effects.apply_damage_reduction(raw_damage);
                        target.hp = (target.hp - damage).max(0);
                        let died = target.hp <= 0;
                        if died {
                            target.die(current_time);
                        }
                        tracing::debug!(
                            "NPC {} hits {} for {} damage (max: {}, raw: {}, HP: {})",
                            npc_id,
                            target_id,
                            damage,
                            max_hit,
                            raw_damage,
                            target.hp
                        );
                        (target.hp, target.x as f32, target.y as f32, died, damage)
                    }
                } else {
                    continue;
                }
            };

            // Broadcast damage event to players in the same zone
            self.broadcast_to_zone(
                &target_id,
                ServerMessage::DamageEvent {
                    source_id: npc_id.clone(),
                    target_id: target_id.clone(),
                    damage,
                    target_hp,
                    target_x,
                    target_y,
                    projectile: None,
                },
            )
            .await;

            // Interrupt crafting if player took damage
            if damage > 0 {
                self.cancel_crafting(&target_id, "interrupted").await;
            }

            // Note: We intentionally do NOT interrupt auto-action when hit by
            // a different NPC. The player chose their target and should stay
            // locked on it (matching OSRS behavior). Auto-action is only
            // cancelled by: player death, target death, explicit cancel, or movement.

            // Auto-retaliate: if the player has no auto-action and auto-retaliate
            // is enabled, automatically fight back against the attacking NPC.
            // Stops after 5 minutes of inactivity (no manual input).
            if !died {
                let should_retaliate = {
                    let players = self.players.read().await;
                    if let Some(player) = players.get(&target_id) {
                        // Only retaliate if no existing auto-action — stick with
                        // current target until it dies or goes out of range
                        player.auto_retaliate
                            && player.auto_action.is_none()
                            && !player.is_dead
                            && player.move_dx == 0 && player.move_dy == 0
                            && player.pending_move_seq.is_none()
                            && current_time.saturating_sub(player.last_move_input_ms) >= 500
                            && current_time.saturating_sub(player.last_activity_time)
                                < AUTO_RETALIATE_IDLE_TIMEOUT_MS
                            // 500ms delay before selecting a new retaliation target
                            && current_time.saturating_sub(player.last_attack_time) >= 500
                    } else {
                        false
                    }
                };
                if should_retaliate {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(&target_id) {
                        player.auto_action = Some(AutoAction {
                            target: AutoActionTarget::Npc {
                                npc_id: npc_id.clone(),
                            },
                            action: AutoActionType::Attack,
                            started_at: current_time,
                        });
                        player.target_id = Some(npc_id.clone());
                    }
                    drop(players);

                    self.send_to_player(
                        &target_id,
                        ServerMessage::AutoActionStarted {
                            target_type: "npc".to_string(),
                            target_id: npc_id.clone(),
                            action: "attack".to_string(),
                        },
                    )
                    .await;

                    self.broadcast_to_zone(
                        &target_id,
                        ServerMessage::TargetChanged {
                            player_id: target_id.clone(),
                            target_id: Some(npc_id.clone()),
                        },
                    )
                    .await;
                }
            }

            // Handle player death
            if died {
                tracing::info!("NPC {} killed player {}", npc_id, target_id);
                self.broadcast_to_zone(
                    &target_id,
                    ServerMessage::PlayerDied {
                        id: target_id.clone(),
                        killer_id: npc_id.clone(),
                    },
                )
                .await;

                self.clear_auto_action(&target_id, "player_died").await;

                // Check KOTH player death
                self.check_koth_player_death(&target_id, current_time).await;

                // Send prayer state update to dying player (prayers cleared on death)
                let (points, max_points) = {
                    let players = self.players.read().await;
                    if let Some(p) = players.get(&target_id) {
                        (p.prayer_points, p.max_prayer_points())
                    } else {
                        (0, 1)
                    }
                };
                self.send_to_player(
                    &target_id,
                    ServerMessage::PrayerStateUpdate {
                        points,
                        max_points,
                        active_prayers: vec![], // Cleared on death
                    },
                )
                .await;
            }
        }

        // Broadcast respawns
        for (id, x, y) in respawned_npcs {
            self.broadcast_to_area(None, x, y, ServerMessage::NpcRespawned { id, x, y })
                .await;
        }

        self.process_auto_action_tick(current_time).await;

        // Check for expired items (60 second lifetime), skip persistent spawns
        let expired_items: Vec<String> = {
            let items = self.ground_items.read().await;
            items
                .iter()
                .filter(|(id, item)| {
                    !id.starts_with("persistent_") && item.is_expired(current_time)
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        // Remove and notify only players who could see the item.
        for item_id in expired_items {
            let mut items = self.ground_items.write().await;
            if let Some(item) = items.remove(&item_id) {
                drop(items);
                self.broadcast_to_area(
                    item.instance_id.as_deref(),
                    item.x.floor() as i32,
                    item.y.floor() as i32,
                    ServerMessage::ItemDespawned { item_id },
                )
                .await;
            }
        }

        // Respawn persistent ground items whose timers have elapsed
        {
            let respawns = {
                let mut gsm = self.ground_spawn_manager.write().await;
                gsm.check_respawns()
            };

            if !respawns.is_empty() {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                for (spawn_id, item_id, x, y, quantity, instance_id) in respawns {
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
                    {
                        let mut items = self.ground_items.write().await;
                        items.insert(ground_item_id.clone(), ground_item.clone());
                    }
                    {
                        let mut gsm = self.ground_spawn_manager.write().await;
                        gsm.set_active_ground_item(&spawn_id, ground_item_id);
                    }
                    self.broadcast_to_area(
                        ground_item.instance_id.as_deref(),
                        ground_item.x.floor() as i32,
                        ground_item.y.floor() as i32,
                        ServerMessage::ItemDropped {
                            id: ground_item.id,
                            item_id: ground_item.item_id,
                            x: ground_item.x,
                            y: ground_item.y,
                            quantity: ground_item.quantity,
                        },
                    )
                    .await;
                    tracing::debug!("Respawned persistent ground item: {}", spawn_id);
                }
            }
        }

        self.sync_ground_item_visibility().await;
        self.process_resource_ticks(current_time).await;

        let farming_growth_ms = self
            .process_farming_growth_updates(current_tick, current_time)
            .await;

        // Check for shop restocks (every 60 seconds)
        {
            let last_restock = *self.last_shop_restock.read().await;
            if last_restock.elapsed().as_secs() >= 60 {
                let restock_start = std::time::Instant::now();
                self.restock_shops().await;
                let mut last = self.last_shop_restock.write().await;
                *last = std::time::Instant::now();
                restock_ms = restock_start.elapsed().as_millis();
            }
        }

        // Tick chest spawn timers (every tick is fine — the check is cheap)
        self.process_chest_respawns().await;

        let npc_world_ms = npc_world_start.elapsed().as_millis();

        // Send state sync to each player, filtering by instance and view distance
        // Snapshot lock data quickly, then release locks before expensive encoding
        let state_sync_start = std::time::Instant::now();
        let tick = *self.tick.read().await;

        let instance_snapshot: HashMap<String, String> = self.player_instances.read().await.clone();
        let senders_snapshot = self.transport.player_senders().await;

        // Build quest lookup snapshots once per tick so StateSync avoids repeated async
        // registry lookups while evaluating per-player quest marker visibility.
        let all_quests = self.quest_registry.all_quests().await;
        let mut quest_by_id = HashMap::new();
        let mut npc_quest_ids: HashMap<String, Vec<String>> = HashMap::new();
        for quest in all_quests {
            if !quest.giver_npc.is_empty() {
                npc_quest_ids
                    .entry(quest.giver_npc.clone())
                    .or_default()
                    .push(quest.id.clone());
            }
            quest_by_id.insert(quest.id.clone(), quest);
        }

        // Per-player lookup: quest giver prototype IDs that should show turn-in check icons.
        // Includes:
        // 1) Quests already in ReadyToComplete
        // 2) Active quests where all non-giver objectives are complete and the only
        //    remaining step is "talk_to" the giver (return-to-giver prompt)
        let ready_turnin_npc_types_by_player: HashMap<String, HashSet<String>> = {
            let mut out: HashMap<String, HashSet<String>> = HashMap::new();

            let quest_states = self.player_quest_states.read().await;
            for (player_id, state) in quest_states.iter() {
                let mut givers: HashSet<String> = HashSet::new();

                for (quest_id, progress) in state.active_quests.iter() {
                    let Some(quest_def) = quest_by_id.get(quest_id) else {
                        continue;
                    };

                    if quest_def.giver_npc.is_empty() {
                        continue;
                    }

                    // Fast path: quest already ready to complete
                    if progress.status == crate::quest::QuestStatus::ReadyToComplete {
                        givers.insert(quest_def.giver_npc.clone());
                        continue;
                    }

                    if progress.status != crate::quest::QuestStatus::Active {
                        continue;
                    }

                    let giver_npc = quest_def.giver_npc.as_str();
                    let mut has_incomplete_return_to_giver = false;
                    let mut all_other_objectives_complete = true;

                    for objective in &quest_def.objectives {
                        let completed = progress
                            .objectives
                            .get(&objective.id)
                            .map(|o| o.completed)
                            .unwrap_or(false);

                        let is_return_to_giver = objective.objective_type == ObjectiveType::TalkTo
                            && objective.target == giver_npc;

                        if is_return_to_giver {
                            if !completed {
                                has_incomplete_return_to_giver = true;
                            }
                        } else if !completed {
                            all_other_objectives_complete = false;
                            break;
                        }
                    }

                    if has_incomplete_return_to_giver && all_other_objectives_complete {
                        givers.insert(quest_def.giver_npc.clone());
                    }
                }

                if !givers.is_empty() {
                    out.insert(player_id.clone(), givers);
                }
            }

            out
        };

        // For merchant+quest_giver NPCs, hide is_quest_giver when all quests are done.
        // Build NPC -> quest IDs, then check per-player completion.
        let all_npc_quests_done_by_player: HashMap<String, HashSet<String>> = {
            let mut out: HashMap<String, HashSet<String>> = HashMap::new();
            let quest_states = self.player_quest_states.read().await;
            for (player_id, state) in quest_states.iter() {
                let mut done_npcs: HashSet<String> = HashSet::new();
                for (npc_id, quest_ids) in &npc_quest_ids {
                    if !quest_ids.is_empty()
                        && quest_ids.iter().all(|qid| state.is_quest_completed(qid))
                    {
                        done_npcs.insert(npc_id.clone());
                    }
                }
                if !done_npcs.is_empty() {
                    out.insert(player_id.clone(), done_npcs);
                }
            }
            out
        };

        // Build position lookup for O(1) access during culling
        let player_pos_map: HashMap<&str, (i32, i32)> = player_updates
            .iter()
            .map(|p| (p.id.as_str(), (p.x, p.y)))
            .collect();

        // Separate overworld vs instance senders
        let mut instance_groups: HashMap<&str, Vec<SenderRef<'_>>> = HashMap::new();
        let mut overworld_senders: Vec<SenderRef<'_>> = Vec::new();
        for (player_id, sender) in senders_snapshot.iter() {
            match instance_snapshot.get(player_id) {
                Some(inst_id) => instance_groups
                    .entry(inst_id.as_str())
                    .or_default()
                    .push((player_id, sender)),
                None => overworld_senders.push((player_id, sender)),
            }
        }

        // Pre-filter player updates by instance
        let mut players_by_instance: HashMap<&str, Vec<&PlayerUpdate>> = HashMap::new();
        let mut overworld_players: Vec<&PlayerUpdate> = Vec::new();
        for p in &player_updates {
            match instance_snapshot.get(&p.id) {
                Some(inst_id) => players_by_instance
                    .entry(inst_id.as_str())
                    .or_default()
                    .push(p),
                None => overworld_players.push(p),
            }
        }

        let current_tick = tick;

        // Instance groups: per-player encode because quest turn-in indicators are player-specific.
        for (inst_id, group_senders) in &instance_groups {
            let mut active_receivers: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
            let mut low_capacity_receivers: Vec<(&String, &mpsc::Sender<Vec<u8>>)> = Vec::new();
            for (pid, sender) in group_senders.iter().copied() {
                if sender.capacity() >= STATE_SYNC_MIN_QUEUE_CAPACITY {
                    active_receivers.push((pid, sender));
                } else {
                    low_capacity_receivers.push((pid, sender));
                }
            }
            tick_telemetry.state_sync_capacity_skips += low_capacity_receivers.len();

            let players_in_instance: Vec<&PlayerUpdate> = players_by_instance
                .get(inst_id)
                .cloned()
                .unwrap_or_default();
            let player_map_in_instance: HashMap<&str, &PlayerUpdate> = players_in_instance
                .iter()
                .map(|p| (p.id.as_str(), *p))
                .collect();

            let instance_npcs: Vec<NpcUpdate> =
                if let Some(instance) = self.instance_manager.get_by_instance_id(inst_id) {
                    instance.get_npc_updates().await
                } else {
                    Vec::new()
                };

            for (pid, sender) in active_receivers {
                let ready_turnin_npcs = ready_turnin_npc_types_by_player.get(pid.as_str());
                let done_npcs = all_npc_quests_done_by_player.get(pid.as_str());
                let current_players: HashMap<String, PlayerUpdate> = players_in_instance
                    .iter()
                    .map(|player| (player.id.clone(), (*player).clone()))
                    .collect();
                let current_npcs: HashMap<String, NpcUpdate> = instance_npcs
                    .iter()
                    .map(|n| {
                        let mut n_for_player = n.clone();
                        // Hide quest giver icon for merchant NPCs whose quests are all done
                        if n_for_player.is_quest_giver
                            && n_for_player.is_merchant
                            && done_npcs
                                .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                                .unwrap_or(false)
                        {
                            n_for_player.is_quest_giver = false;
                        }
                        n_for_player.can_turn_in_quest = n_for_player.is_quest_giver
                            && ready_turnin_npcs
                                .map(|set| set.contains(n_for_player.prototype_id.as_str()))
                                .unwrap_or(false);
                        (n_for_player.id.clone(), n_for_player)
                    })
                    .collect();

                let Some(mut sync_state) = self.transport.sync_state(pid.as_str()) else {
                    continue;
                };
                sync_state.ensure_context(inst_id);
                let needs_full = sync_state.last_full_sync_tick == 0
                    || current_tick >= sync_state.next_full_sync_tick;

                if needs_full {
                    let player_values = current_players
                        .values()
                        .map(crate::protocol::player_update_to_value)
                        .collect();
                    let npc_values = current_npcs
                        .values()
                        .map(crate::protocol::npc_update_to_value)
                        .collect();

                    if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                        tick,
                        player_values,
                        npc_values,
                        inst_id,
                    ) {
                        let raw_len = raw.len();
                        let bytes = crate::protocol::maybe_compress(raw);
                        let bytes_len = bytes.len();
                        tick_telemetry.state_sync_send_attempts += 1;
                        tick_telemetry.state_sync_full_sends += 1;
                        tick_telemetry.state_sync_raw_bytes += raw_len;
                        tick_telemetry.state_sync_bytes_sent += bytes_len;
                        if let Err(e) = sender.try_send(bytes) {
                            tick_telemetry.state_sync_try_send_drops += 1;
                            tracing::debug!("StateSync drop for {}: {}", pid, e);
                        } else {
                            let initial_offset = if sync_state.last_full_sync_tick == 0 {
                                full_sync_offset(pid)
                            } else {
                                0
                            };
                            sync_state.last_full_sync_tick = current_tick;
                            sync_state.next_full_sync_tick =
                                current_tick + FULL_SYNC_INTERVAL + initial_offset;
                            sync_state.last_players = current_players;
                            sync_state.last_npcs = current_npcs;
                        }
                    }
                } else {
                    let changed_players = current_players
                        .iter()
                        .filter(|(id, update)| {
                            id.as_str() == pid.as_str()
                                || sync_state.last_players.get(id.as_str()) != Some(*update)
                        })
                        .map(|(_, update)| crate::protocol::player_update_to_value(update))
                        .collect();
                    let changed_npcs = current_npcs
                        .iter()
                        .filter(|(id, update)| {
                            sync_state.last_npcs.get(id.as_str()) != Some(*update)
                        })
                        .map(|(_, update)| crate::protocol::npc_update_to_value(update))
                        .collect();
                    let removed_players: Vec<String> = sync_state
                        .last_players
                        .keys()
                        .filter(|id| !current_players.contains_key(id.as_str()))
                        .cloned()
                        .collect();
                    let removed_npcs: Vec<String> = sync_state
                        .last_npcs
                        .keys()
                        .filter(|id| !current_npcs.contains_key(id.as_str()))
                        .cloned()
                        .collect();

                    if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                        tick,
                        changed_players,
                        changed_npcs,
                        inst_id,
                        false,
                        &removed_players,
                        &removed_npcs,
                    ) {
                        let raw_len = raw.len();
                        let bytes = crate::protocol::maybe_compress(raw);
                        let bytes_len = bytes.len();
                        tick_telemetry.state_sync_send_attempts += 1;
                        tick_telemetry.state_sync_delta_sends += 1;
                        tick_telemetry.state_sync_raw_bytes += raw_len;
                        tick_telemetry.state_sync_bytes_sent += bytes_len;
                        if let Err(e) = sender.try_send(bytes) {
                            tick_telemetry.state_sync_try_send_drops += 1;
                            tracing::debug!("StateSync delta drop for {}: {}", pid, e);
                        } else {
                            sync_state.last_players = current_players;
                            sync_state.last_npcs = current_npcs;
                        }
                    }
                }
            }

            // If a client is under queue pressure, still try to send a tiny self-only delta
            // so local correction/facing remains responsive during transient congestion.
            for (pid, sender) in low_capacity_receivers {
                if sender.capacity() == 0 {
                    continue;
                }
                let Some(self_update) = player_map_in_instance.get(pid.as_str()) else {
                    continue;
                };
                let self_values = vec![crate::protocol::player_update_to_value(self_update)];
                if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                    tick,
                    self_values,
                    Vec::new(),
                    inst_id,
                    false,
                    &[],
                    &[],
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_delta_sends += 1;
                    tick_telemetry.state_sync_fallback_self_only_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync fallback drop for {}: {}", pid, e);
                    }
                }
            }
        }

        // Overworld: delta-compressed per-player StateSync
        // Build lookup maps for nearby entity filtering
        let overworld_player_map: HashMap<&str, &PlayerUpdate> = overworld_players
            .iter()
            .map(|p| (p.id.as_str(), *p))
            .collect();
        let mut overworld_players_by_chunk: HashMap<(i32, i32), Vec<&PlayerUpdate>> =
            HashMap::new();
        for player in &overworld_players {
            overworld_players_by_chunk
                .entry((
                    player.x.div_euclid(CHUNK_SIZE as i32),
                    player.y.div_euclid(CHUNK_SIZE as i32),
                ))
                .or_default()
                .push(*player);
        }
        let mut npcs_by_chunk: HashMap<(i32, i32), Vec<&NpcUpdate>> = HashMap::new();
        for npc in &npc_updates {
            let min_chunk_x = npc.x.div_euclid(CHUNK_SIZE as i32);
            let min_chunk_y = npc.y.div_euclid(CHUNK_SIZE as i32);
            let max_chunk_x = (npc.x + npc.size.max(1) - 1).div_euclid(CHUNK_SIZE as i32);
            let max_chunk_y = (npc.y + npc.size.max(1) - 1).div_euclid(CHUNK_SIZE as i32);
            for chunk_x in min_chunk_x..=max_chunk_x {
                for chunk_y in min_chunk_y..=max_chunk_y {
                    npcs_by_chunk
                        .entry((chunk_x, chunk_y))
                        .or_default()
                        .push(npc);
                }
            }
        }
        let sync_chunk_radius = (VIEW_DISTANCE + CHUNK_SIZE as i32 - 1) / CHUNK_SIZE as i32;

        for (player_id, sender) in &overworld_senders {
            if sender.capacity() < STATE_SYNC_MIN_QUEUE_CAPACITY {
                tick_telemetry.state_sync_capacity_skips += 1;
                if sender.capacity() > 0
                    && let Some(self_update) = overworld_player_map.get(player_id.as_str())
                {
                    let self_values = vec![crate::protocol::player_update_to_value(self_update)];
                    if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                        tick,
                        self_values,
                        Vec::new(),
                        "",
                        false,
                        &[],
                        &[],
                    ) {
                        let raw_len = raw.len();
                        let bytes = crate::protocol::maybe_compress(raw);
                        let bytes_len = bytes.len();
                        tick_telemetry.state_sync_send_attempts += 1;
                        tick_telemetry.state_sync_delta_sends += 1;
                        tick_telemetry.state_sync_fallback_self_only_sends += 1;
                        tick_telemetry.state_sync_raw_bytes += raw_len;
                        tick_telemetry.state_sync_bytes_sent += bytes_len;
                        if let Err(e) = sender.try_send(bytes) {
                            tick_telemetry.state_sync_try_send_drops += 1;
                            tracing::debug!("StateSync fallback drop for {}: {}", player_id, e);
                        }
                    }
                }
                continue;
            }

            let (px, py) = match player_pos_map.get(player_id.as_str()) {
                Some(pos) => *pos,
                None => continue,
            };

            // Filter nearby entities by view distance
            let center_chunk_x = px.div_euclid(CHUNK_SIZE as i32);
            let center_chunk_y = py.div_euclid(CHUNK_SIZE as i32);
            let mut nearby_players: HashMap<String, &PlayerUpdate> = HashMap::new();
            let mut nearby_npcs: HashMap<String, NpcUpdate> = HashMap::new();

            for chunk_x in
                (center_chunk_x - sync_chunk_radius)..=(center_chunk_x + sync_chunk_radius)
            {
                for chunk_y in
                    (center_chunk_y - sync_chunk_radius)..=(center_chunk_y + sync_chunk_radius)
                {
                    if let Some(players) = overworld_players_by_chunk.get(&(chunk_x, chunk_y)) {
                        for player in players {
                            if (player.x - px).abs().max((player.y - py).abs()) <= VIEW_DISTANCE {
                                nearby_players.insert(player.id.clone(), *player);
                            }
                        }
                    }

                    if let Some(npcs) = npcs_by_chunk.get(&(chunk_x, chunk_y)) {
                        for npc in npcs {
                            let closest_x = px.clamp(npc.x, npc.x + npc.size.max(1) - 1);
                            let closest_y = py.clamp(npc.y, npc.y + npc.size.max(1) - 1);
                            if (closest_x - px).abs().max((closest_y - py).abs()) <= VIEW_DISTANCE {
                                nearby_npcs
                                    .entry(npc.id.clone())
                                    .or_insert_with(|| (*npc).clone());
                            }
                        }
                    }
                }
            }

            let ready_turnin_npcs = ready_turnin_npc_types_by_player.get(player_id.as_str());
            let done_npcs = all_npc_quests_done_by_player.get(player_id.as_str());
            for npc in nearby_npcs.values_mut() {
                if npc.is_quest_giver
                    && npc.is_merchant
                    && done_npcs
                        .map(|set| set.contains(npc.prototype_id.as_str()))
                        .unwrap_or(false)
                {
                    npc.is_quest_giver = false;
                }
                npc.can_turn_in_quest = npc.is_quest_giver
                    && ready_turnin_npcs
                        .map(|set| set.contains(npc.prototype_id.as_str()))
                        .unwrap_or(false);
            }

            let Some(mut sync_state) = self.transport.sync_state(player_id.as_str()) else {
                continue;
            };
            sync_state.ensure_context("");
            let needs_full = sync_state.last_full_sync_tick == 0
                || current_tick >= sync_state.next_full_sync_tick;

            if needs_full {
                // Full sync: encode all nearby entities
                let player_values: Vec<rmpv::Value> = nearby_players
                    .values()
                    .map(|p| crate::protocol::player_update_to_value(p))
                    .collect();
                let npc_values: Vec<rmpv::Value> = nearby_npcs
                    .values()
                    .map(crate::protocol::npc_update_to_value)
                    .collect();

                if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                    tick,
                    player_values,
                    npc_values,
                    "",
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_full_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync drop for {}: {}", player_id, e);
                    } else {
                        // Only update sync state if send succeeded
                        let initial_offset = if sync_state.last_full_sync_tick == 0 {
                            full_sync_offset(player_id)
                        } else {
                            0
                        };
                        sync_state.last_full_sync_tick = current_tick;
                        sync_state.next_full_sync_tick =
                            current_tick + FULL_SYNC_INTERVAL + initial_offset;
                        sync_state.last_players = nearby_players
                            .into_iter()
                            .map(|(id, p)| (id, p.clone()))
                            .collect();
                        sync_state.last_npcs = nearby_npcs;
                    }
                }
            } else {
                // Delta sync: only encode changed/new entities + removal lists
                let mut changed_players: Vec<rmpv::Value> = Vec::new();
                for (id, update) in &nearby_players {
                    // Always include the receiving player's own update — the client
                    // needs continuous position confirmation to correct mispredictions
                    // (e.g. rejected moves hitting walls/NPCs).
                    if id == player_id.as_str() {
                        changed_players.push(crate::protocol::player_update_to_value(update));
                        continue;
                    }
                    match sync_state.last_players.get(id) {
                        Some(last) if last == *update => {} // unchanged, skip
                        _ => changed_players.push(crate::protocol::player_update_to_value(update)),
                    }
                }

                let mut changed_npcs: Vec<rmpv::Value> = Vec::new();
                for (id, update) in &nearby_npcs {
                    match sync_state.last_npcs.get(id) {
                        Some(last) if last == update => {} // unchanged, skip
                        _ => changed_npcs.push(crate::protocol::npc_update_to_value(update)),
                    }
                }

                // Find removed entities (were in last sync but not nearby now)
                let removed_players: Vec<String> = sync_state
                    .last_players
                    .keys()
                    .filter(|id| !nearby_players.contains_key(id.as_str()))
                    .cloned()
                    .collect();
                let removed_npcs: Vec<String> = sync_state
                    .last_npcs
                    .keys()
                    .filter(|id| !nearby_npcs.contains_key(id.as_str()))
                    .cloned()
                    .collect();

                if let Ok(raw) = crate::protocol::encode_delta_state_sync(
                    tick,
                    changed_players,
                    changed_npcs,
                    "",
                    false,
                    &removed_players,
                    &removed_npcs,
                ) {
                    let raw_len = raw.len();
                    let bytes = crate::protocol::maybe_compress(raw);
                    let bytes_len = bytes.len();
                    tick_telemetry.state_sync_send_attempts += 1;
                    tick_telemetry.state_sync_delta_sends += 1;
                    tick_telemetry.state_sync_raw_bytes += raw_len;
                    tick_telemetry.state_sync_bytes_sent += bytes_len;
                    if let Err(e) = sender.try_send(bytes) {
                        tick_telemetry.state_sync_try_send_drops += 1;
                        tracing::debug!("StateSync drop for {}: {}", player_id, e);
                    } else {
                        // Keep a rolling baseline on successful deltas to avoid resending
                        // the same changed entities on every tick.
                        sync_state.last_players = nearby_players
                            .iter()
                            .map(|(id, p)| (id.clone(), (*p).clone()))
                            .collect();
                        sync_state.last_npcs = nearby_npcs.clone();
                    }
                }
            }
        }

        // === Spectator StateSync ===
        // Generate a single StateSync for all spectators, centered on world spawn.
        // Spectators are read-only observers so we skip quest turn-in icons and delta
        // compression — one full encode shared across every spectator connection.
        let spectator_senders = self.transport.spectator_senders().await;
        let spectator_count = spectator_senders.len();
        if !spectator_senders.is_empty() {
            // Gather players near spawn with VIEW_DISTANCE culling
            let mut spectator_player_values: Vec<rmpv::Value> = Vec::new();
            for p in &overworld_players {
                let dx = (p.x - WORLD_SPAWN_X).abs();
                let dy = (p.y - WORLD_SPAWN_Y).abs();
                if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
                    spectator_player_values.push(crate::protocol::player_update_to_value(p));
                }
            }

            // Gather NPCs near spawn
            let mut spectator_npc_values: Vec<rmpv::Value> = Vec::new();
            for n in &npc_updates {
                let dx = (n.x - WORLD_SPAWN_X).abs();
                let dy = (n.y - WORLD_SPAWN_Y).abs();
                if dx <= VIEW_DISTANCE && dy <= VIEW_DISTANCE {
                    spectator_npc_values.push(crate::protocol::npc_update_to_value(n));
                }
            }

            // Encode once for all spectators (always full sync, no delta tracking)
            if let Ok(raw) = crate::protocol::encode_state_sync_from_values(
                current_tick,
                spectator_player_values,
                spectator_npc_values,
                "",
            ) {
                let bytes = crate::protocol::maybe_compress(raw);
                for sender in spectator_senders.values() {
                    let _ = sender.try_send(bytes.clone());
                }
            }
        }
        let state_sync_ms = state_sync_start.elapsed().as_millis();

        // Cancel trades if players moved too far apart
        {
            let trade_ids: Vec<String> = {
                let trades = self.trades.read().await;
                trades.keys().cloned().collect()
            };
            for trade_id in trade_ids {
                let participants = {
                    let trades = self.trades.read().await;
                    trades
                        .get(&trade_id)
                        .map(|session| (session.player_a.clone(), session.player_b.clone()))
                };
                let should_cancel = if let Some((player_a, player_b)) = participants {
                    let players = self.players.read().await;
                    match (players.get(&player_a), players.get(&player_b)) {
                        (Some(a), Some(b)) => {
                            let dx = (a.x - b.x).abs();
                            let dy = (a.y - b.y).abs();
                            dx > TRADE_MAX_DISTANCE
                                || dy > TRADE_MAX_DISTANCE
                                || a.is_dead
                                || b.is_dead
                                || !a.active
                                || !b.active
                        }
                        _ => true,
                    }
                } else {
                    false
                };
                if should_cancel {
                    let session = {
                        let mut trades = self.trades.write().await;
                        trades.remove(&trade_id)
                    };
                    if let Some(session) = session {
                        {
                            let mut pt = self.player_trades.write().await;
                            pt.remove(&session.player_a);
                            pt.remove(&session.player_b);
                        }
                        let msg = ServerMessage::TradeCancelled {
                            reason: "Too far apart.".to_string(),
                        };
                        self.send_to_player(&session.player_a, msg.clone()).await;
                        self.send_to_player(&session.player_b, msg).await;
                    }
                }
            }
        }

        // Expire old trade requests (20 second timeout)
        {
            let mut requests = self.trade_requests.write().await;
            requests.retain(|_, (_, tick)| current_tick - *tick < 400);
        }

        // Close stall if player died
        {
            let stall_owners: Vec<String> = {
                let players = self.players.read().await;
                players
                    .values()
                    .filter(|p| p.stall.as_ref().is_some_and(|s| s.active) && p.is_dead)
                    .map(|p| p.id.clone())
                    .collect()
            };
            for pid in stall_owners {
                self.force_close_stall(&pid).await;
                self.send_to_player(
                    &pid,
                    ServerMessage::StallClosed {
                        reason: "Shop closed (you died).".to_string(),
                    },
                )
                .await;
            }
        }

        // KOTH tick: wave spawning + phase transitions
        self.process_koth_tick(current_time).await;

        // Boss fight tick
        self.process_boss_tick(current_time).await;
        self.process_pharaoh_boss_tick(current_time).await;
        self.process_reaper_boss_tick(current_time).await;

        // Arena tick: zone detection + state machine
        let arena_start = std::time::Instant::now();
        self.arena_tick(current_time).await;
        let arena_ms = arena_start.elapsed().as_millis();

        tick_telemetry.active_players = senders_snapshot.len();
        tick_telemetry.overworld_players = overworld_senders.len();
        tick_telemetry.instance_players =
            instance_groups.values().map(|senders| senders.len()).sum();
        tick_telemetry.spectators = spectator_count;
        tick_telemetry.pre_npc_ms = pre_npc_ms as u64;
        tick_telemetry.npc_world_ms = npc_world_ms as u64;
        tick_telemetry.state_sync_ms = state_sync_ms as u64;
        tick_telemetry.arena_ms = arena_ms as u64;
        tick_telemetry.chunk_unload_ms = chunk_unload_ms as u64;
        tick_telemetry.prayer_drain_ms = prayer_drain_ms as u64;
        tick_telemetry.farming_growth_ms = farming_growth_ms as u64;
        tick_telemetry.restock_ms = restock_ms as u64;

        // Log slow ticks for debugging latency spikes
        let tick_duration = tick_start.elapsed();
        if tick_duration.as_millis() > 50 {
            tracing::warn!(
                "Slow tick {}: {}ms (pre_npc={}ms npc_world={}ms sync={}ms arena={}ms players={} npcs={} overworld_senders={} instance_groups={} chunk_unload={}ms prayer_drain={}ms farming_growth={}ms restock={}ms moves={}/{} reject_reasons(tile={} player={} npc={} chair={} arena={}) sync_attempts={} sync_capacity_skips={} sync_drops={} sync_full={} sync_delta={} sync_fallback={} sync_raw_bytes={} sync_wire_bytes={})",
                current_tick,
                tick_duration.as_millis(),
                pre_npc_ms,
                npc_world_ms,
                state_sync_ms,
                arena_ms,
                player_updates.len(),
                npc_updates.len(),
                overworld_senders.len(),
                instance_groups.len(),
                chunk_unload_ms,
                prayer_drain_ms,
                farming_growth_ms,
                restock_ms,
                tick_telemetry
                    .pending_moves
                    .saturating_sub(tick_telemetry.rejected_moves),
                tick_telemetry.pending_moves,
                tick_telemetry.rejected_tile_blocked,
                tick_telemetry.rejected_player_blocked,
                tick_telemetry.rejected_npc_blocked,
                tick_telemetry.rejected_chair_blocked,
                tick_telemetry.rejected_arena_blocked,
                tick_telemetry.state_sync_send_attempts,
                tick_telemetry.state_sync_capacity_skips,
                tick_telemetry.state_sync_try_send_drops,
                tick_telemetry.state_sync_full_sends,
                tick_telemetry.state_sync_delta_sends,
                tick_telemetry.state_sync_fallback_self_only_sends,
                tick_telemetry.state_sync_raw_bytes,
                tick_telemetry.state_sync_bytes_sent,
            );
        }

        tick_telemetry
    }
}
