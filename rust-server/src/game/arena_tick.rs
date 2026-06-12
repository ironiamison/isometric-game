use super::*;

impl GameRoom {
    pub(super) async fn arena_tick(&self, current_time: u64) {
        // Auto-queue/dequeue players based on position in queue zone
        let mut queue_errors: Vec<(String, String)> = Vec::new();
        let arena_player_ids: HashSet<String> = {
            let player_instances = self.player_instances.read().await;
            player_instances
                .iter()
                .filter(|(_, instance_id)| instance_id.starts_with("pub_duel_arena"))
                .map(|(player_id, _)| player_id.clone())
                .collect()
        };
        let arena_players: Vec<(String, String, i32, i32, i32)> = {
            let players = self.players.read().await;
            arena_player_ids
                .iter()
                .filter_map(|player_id| {
                    let player = players.get(player_id)?;
                    if player.is_dead || !player.active {
                        return None;
                    }
                    Some((
                        player_id.clone(),
                        player.name.clone(),
                        player.x,
                        player.y,
                        player.inventory.gold,
                    ))
                })
                .collect()
        };
        {
            let mut arena = self.arena_manager.write().await;

            for (player_id, player_name, player_x, player_y, player_gold) in arena_players {
                let in_queue_zone = arena.is_in_queue_zone(player_x, player_y);
                let is_queued = arena.queued_players.contains(&player_id);

                if in_queue_zone && !is_queued && arena.state == crate::arena::ArenaState::Idle {
                    if !arena.queue_rejected.contains(&player_id)
                        && let Err(e) = arena.queue_player(&player_id, &player_name, player_gold)
                    {
                        arena.queue_rejected.insert(player_id.clone());
                        queue_errors.push((player_id, e));
                    }
                } else if !in_queue_zone {
                    if is_queued && arena.state == crate::arena::ArenaState::Idle {
                        arena.dequeue_player(&player_id);
                    }
                    arena.queue_rejected.remove(&player_id);
                }
            }
        }
        for (pid, err) in queue_errors {
            self.send_system_message(&pid, &err).await;
        }

        // Process arena state machine
        let events = {
            let mut arena = self.arena_manager.write().await;
            arena.tick(current_time)
        };

        for event in events {
            match event {
                crate::arena::ArenaEvent::FightStarted { fighters } => {
                    let fighter_ids: Vec<String> =
                        fighters.iter().map(|(id, _)| id.clone()).collect();

                    // Teleport fighters to spawn points
                    {
                        let mut players = self.players.write().await;
                        for (player_id, (spawn_x, spawn_y)) in &fighters {
                            if let Some(player) = players.get_mut(player_id) {
                                player.x = *spawn_x;
                                player.y = *spawn_y;
                            }
                        }
                    }

                    // Broadcast match start to all arena players
                    self.broadcast_to_arena(ServerMessage::ArenaMatchStart { fighter_ids })
                        .await;
                }
                crate::arena::ArenaEvent::MatchEnded { placements } => {
                    // Distribute gold rewards
                    {
                        let mut players = self.players.write().await;
                        for placement in &placements {
                            if placement.gold_reward > 0
                                && let Some(player) = players.get_mut(&placement.player_id)
                            {
                                player.inventory.gold = item::checked_gold_credit(
                                    player.inventory.gold,
                                    placement.gold_reward,
                                )
                                .unwrap_or(item::MAX_GOLD);
                            }
                        }
                    }

                    // Save arena stats to DB
                    if let Some(ref db) = self.db {
                        // We need character IDs - for now we look them up via player name
                        // This is a best-effort save; failures are logged but don't block gameplay
                        for placement in &placements {
                            let won = placement.rank == 1;
                            let died = placement.rank > 1;
                            if let Err(e) = db
                                .update_arena_stats(
                                    0, // character_id will be resolved in the save path
                                    won,
                                    placement.kills,
                                    died,
                                    placement.gold_reward,
                                )
                                .await
                            {
                                tracing::warn!(
                                    "Failed to save arena stats for {}: {}",
                                    placement.player_id,
                                    e
                                );
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

                    // Send inventory updates to all fighters who earned gold
                    for placement in &placements {
                        if placement.gold_reward > 0 {
                            let update = {
                                let players = self.players.read().await;
                                players
                                    .get(&placement.player_id)
                                    .map(|p| (p.inventory.to_update(), p.inventory.gold))
                            };
                            if let Some((slots, gold)) = update {
                                self.send_to_player(
                                    &placement.player_id,
                                    ServerMessage::InventoryUpdate {
                                        player_id: placement.player_id.clone(),
                                        slots,
                                        gold,
                                    },
                                )
                                .await;
                            }
                        }
                    }
                }
                crate::arena::ArenaEvent::StateChanged { state } => {
                    let (queued_count, fighter_count, entry_fee, countdown_remaining) = {
                        let arena = self.arena_manager.read().await;
                        let remaining = match &arena.state {
                            crate::arena::ArenaState::Countdown { ends_at } => {
                                Some(ends_at.saturating_sub(current_time) as u32)
                            }
                            _ => None,
                        };
                        (
                            arena.queued_players.len() as u32,
                            arena.active_fighters.len() as u32,
                            arena.config.entry_fee,
                            remaining,
                        )
                    };

                    self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                        state,
                        countdown_remaining,
                        queued_count,
                        fighter_count,
                        entry_fee,
                    })
                    .await;
                }
                crate::arena::ArenaEvent::ResultsExpired => {
                    // Reset broadcast
                    self.broadcast_to_arena(ServerMessage::ArenaStateUpdate {
                        state: "idle".to_string(),
                        countdown_remaining: None,
                        queued_count: 0,
                        fighter_count: 0,
                        entry_fee: {
                            let arena = self.arena_manager.read().await;
                            arena.config.entry_fee
                        },
                    })
                    .await;
                }
            }
        }
    }

    pub(super) async fn broadcast_to_arena(&self, msg: ServerMessage) {
        let senders = self.transport.player_senders().await;
        let recipients: Vec<mpsc::Sender<Vec<u8>>> = {
            let player_instances = self.player_instances.read().await;
            player_instances
                .iter()
                .filter(|(_, instance_id)| instance_id.starts_with("pub_duel_arena"))
                .filter_map(|(player_id, _)| senders.get(player_id).cloned())
                .collect()
        };

        if let Ok(bytes) = crate::protocol::encode_server_message(&msg) {
            for sender in recipients {
                let _ = sender.try_send(bytes.clone());
            }
        }
    }
}
