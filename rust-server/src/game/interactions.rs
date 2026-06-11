use super::*;

impl GameRoom {
    pub async fn handle_npc_interact(&self, player_id: &str, npc_id: &str) {
        // Get player position
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => (p.x, p.y),
                _ => return,
            }
        };

        // Check if player is in an instance
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        // Get NPC info - check instance NPCs first, then overworld NPCs
        let npc_info = if let Some(ref inst_id) = instance_id {
            // Player is in an instance - look up instance NPCs
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    let entity_type = npc.prototype_id.clone();
                    (entity_type, distance, npc.is_alive())
                })
            } else {
                tracing::warn!(
                    "Player {} in instance {} but instance not found",
                    player_id,
                    inst_id
                );
                None
            }
        } else {
            // Player is in overworld - check room NPCs
            let npcs = self.npcs.read().await;
            npcs.get(npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                let entity_type = npc.prototype_id.clone();
                (entity_type, distance, npc.is_alive())
            })
        };

        let (entity_type, distance, is_alive): (String, f32, bool) = match npc_info {
            Some(info) => info,
            None => {
                tracing::warn!(
                    "Player {} tried to interact with unknown NPC {}",
                    player_id,
                    npc_id
                );
                return;
            }
        };

        // Must be within interaction range (2 tiles) and NPC must be alive
        if distance > 2.5 || !is_alive {
            tracing::debug!(
                "Player {} can't interact with NPC {} (distance: {}, alive: {})",
                player_id,
                npc_id,
                distance,
                is_alive
            );
            return;
        }

        self.npc_interaction_grants.write().await.insert(
            player_id.to_string(),
            NpcInteractionGrant {
                npc_id: npc_id.to_string(),
                instance_id,
            },
        );

        // Arena leaderboard interaction
        if entity_type == "arena_board" {
            if let Some(ref db) = self.db {
                match db.get_arena_leaderboard().await {
                    Ok(entries) => {
                        let mut text = String::from("=== Arena Leaderboard ===\n\n");
                        if entries.is_empty() {
                            text.push_str("No arena matches recorded yet.");
                        } else {
                            text.push_str("Rank | Name | Wins | Kills | Gold Won\n");
                            for (i, (name, kills, wins, gold)) in entries.iter().enumerate() {
                                text.push_str(&format!(
                                    "#{} | {} | {} | {} | {}\n",
                                    i + 1,
                                    name,
                                    wins,
                                    kills,
                                    gold
                                ));
                            }
                        }
                        self.send_to_player(
                            player_id,
                            ServerMessage::ShowDialogue {
                                quest_id: String::new(),
                                npc_id: npc_id.to_string(),
                                speaker: "Arena Leaderboard".to_string(),
                                text,
                                choices: vec![crate::protocol::DialogueChoice {
                                    id: "close".to_string(),
                                    text: "Close".to_string(),
                                }],
                            },
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch arena leaderboard: {}", e);
                        self.send_system_message(player_id, "Failed to load leaderboard.")
                            .await;
                    }
                }
            }
            return;
        }

        if entity_type == "adventure_board" {
            self.show_adventure_board_dialogue(player_id, &npc_id).await;
            return;
        }

        if entity_type == "master_artisan" {
            self.show_prestige_shop_dialogue(player_id, &npc_id).await;
            return;
        }

        let is_altar = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.altar)
            .unwrap_or(false);

        if is_altar {
            self.show_altar_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }

        // Plot seller interaction - show plot purchase dialogue
        let is_plot_seller = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.plot_seller)
            .unwrap_or(false);

        if is_plot_seller {
            self.show_master_farmer_dialogue(player_id, &npc_id).await;
            return;
        }

        // Banker interaction - open bank vault
        let is_banker = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.banker)
            .unwrap_or(false);

        if is_banker {
            // Skip dialogue and open bank directly if fully upgraded
            let fully_upgraded = {
                let players = self.players.read().await;
                players
                    .get(player_id)
                    .map(|p| p.bank_max_slots >= item::BANK_MAX_SIZE as u32)
                    .unwrap_or(false)
            };
            if fully_upgraded {
                self.handle_bank_open(player_id).await;
            } else {
                self.show_banker_dialogue(player_id, &npc_id).await;
            }
            return;
        }

        // Slayer master interaction - open slayer panel
        let is_slayer_master = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.slayer_master)
            .unwrap_or(false);

        if is_slayer_master {
            self.handle_slayer_master_interact(player_id, &entity_type)
                .await;
            return;
        }

        // KOTH rewards NPC - show pending rewards
        let is_koth_rewards = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.koth_rewards)
            .unwrap_or(false);

        if is_koth_rewards {
            self.show_koth_rewards_dialogue(player_id, &npc_id).await;
            return;
        }

        // Boss rewards NPC - show pending boss loot
        let is_boss_rewards = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.boss_rewards)
            .unwrap_or(false);

        if is_boss_rewards {
            self.show_boss_rewards_dialogue(player_id, &npc_id).await;
            return;
        }

        // Port master interaction - show travel destinations
        let is_port_master = self
            .entity_registry
            .get(&entity_type)
            .map(|p| p.behaviors.port_master)
            .unwrap_or(false);

        if is_port_master {
            self.show_port_master_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }

        if self
            .resource_contract_kind_for_entity(&entity_type)
            .is_some()
            && self
                .resource_contract_npc_unlocked(player_id, &entity_type)
                .await
        {
            self.show_resource_contract_master_dialogue(player_id, &npc_id, &entity_type)
                .await;
            return;
        }

        if self
            .try_open_merchant_shop(player_id, &npc_id, &entity_type)
            .await
        {
            return;
        }

        self.handle_npc_quest_interaction(player_id, npc_id, &entity_type)
            .await;
    }

    pub async fn handle_use_item_on(&self, player_id: &str, slot_index: u8, target_npc_id: &str) {
        // 1. Get player position and item from inventory
        let (player_x, player_y, item_id) = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return;
            };
            let item_id = player
                .inventory
                .slots
                .get(slot_index as usize)
                .and_then(|s| s.as_ref())
                .map(|s| s.item_id.clone());
            (player.x, player.y, item_id)
        };

        let Some(item_id) = item_id else {
            tracing::warn!(
                "UseItemOn: empty inventory slot {} for {}",
                slot_index,
                player_id
            );
            return;
        };

        tracing::info!(
            "UseItemOn: player={} item={} target_npc={}",
            player_id,
            item_id,
            target_npc_id
        );

        // 2. Get NPC info (check instance first, then overworld)
        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        let npc_info = if instance_id.is_some() {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(target_npc_id).map(|npc| {
                    let dx = (npc.x - player_x) as f32;
                    let dy = (npc.y - player_y) as f32;
                    (
                        npc.prototype_id.clone(),
                        npc.id.clone(),
                        (dx * dx + dy * dy).sqrt(),
                    )
                })
            } else {
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(target_npc_id).map(|npc| {
                let dx = (npc.x - player_x) as f32;
                let dy = (npc.y - player_y) as f32;
                (
                    npc.prototype_id.clone(),
                    npc.id.clone(),
                    (dx * dx + dy * dy).sqrt(),
                )
            })
        };

        let Some((entity_type, _npc_runtime_id, distance)) = npc_info else {
            tracing::warn!(
                "UseItemOn: NPC {} not found (instance_id={:?})",
                target_npc_id,
                instance_id
            );
            return;
        };

        tracing::info!(
            "UseItemOn: found NPC entity_type={} distance={:.1}",
            entity_type,
            distance
        );

        // 3. Range check (same as NPC interaction: 2.5 tiles)
        if distance > 2.5 {
            tracing::info!("UseItemOn: out of range ({:.1} > 2.5)", distance);
            return;
        }

        // 4. Try quest Lua handlers
        let handled = self
            .handle_use_item_on_quest(player_id, &item_id, &entity_type, target_npc_id)
            .await;

        if !handled {
            // 5. TODO: TOML item_interactions fallback
            // 6. Nothing matched — send a generic message
            self.send_to_player(
                player_id,
                ServerMessage::ChatMessage {
                    sender_id: "system".to_string(),
                    sender_name: String::new(),
                    text: "Nothing interesting happens.".to_string(),
                    timestamp: 0,
                    channel: "system".to_string(),
                },
            )
            .await;
        }
    }

    pub async fn handle_dialogue_choice(&self, player_id: &str, quest_id: &str, choice_id: &str) {
        if !self
            .authorize_dialogue_choice(player_id, quest_id, choice_id)
            .await
        {
            tracing::warn!(
                "Rejected unauthorized dialogue choice from {}: quest={}, choice={}",
                player_id,
                quest_id,
                choice_id
            );
            return;
        }

        // Non-quest dialogues (e.g. leaderboard) just close
        if quest_id.is_empty() {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            return;
        }

        if let Some(altar_id) = quest_id.strip_prefix("altar:") {
            self.handle_altar_dialogue_choice(player_id, altar_id, choice_id)
                .await;
            return;
        }

        if let Some(npc_id) = quest_id.strip_prefix("prestige_shop:") {
            self.handle_prestige_shop_choice(player_id, npc_id, choice_id)
                .await;
            return;
        }

        if let Some(npc_id) = quest_id.strip_prefix("adventure_board:") {
            if let Some(rest) = choice_id.strip_prefix("board_accept:") {
                if let Some((kind_str, diff_str)) = rest.split_once(':') {
                    self.handle_accept_adventure_board_contract(
                        player_id, npc_id, kind_str, diff_str,
                    )
                    .await;
                    self.show_adventure_board_dialogue(player_id, npc_id).await;
                }
            } else if choice_id == "board_claim" {
                self.handle_claim_resource_contract(player_id).await;
                self.show_adventure_board_dialogue(player_id, npc_id).await;
            } else if choice_id == "board_abandon" {
                self.handle_abandon_resource_contract(player_id).await;
                self.show_adventure_board_dialogue(player_id, npc_id).await;
            } else if let Some(order_id) = choice_id.strip_prefix("order_accept:") {
                self.handle_accept_crafting_order(player_id, order_id).await;
                self.show_adventure_board_dialogue(player_id, npc_id).await;
            } else if choice_id == "order_claim" {
                self.handle_claim_crafting_order(player_id).await;
                self.show_adventure_board_dialogue(player_id, npc_id).await;
            } else if choice_id == "order_abandon" {
                self.handle_abandon_crafting_order(player_id).await;
                self.show_adventure_board_dialogue(player_id, npc_id).await;
            } else {
                match choice_id {
                    "board_farming" => {
                        self.show_adventure_board_contract_dialogue(
                            player_id,
                            npc_id,
                            crate::resource_contracts::ResourceContractKind::Farming,
                        )
                        .await;
                    }
                    "board_mining" => {
                        self.show_adventure_board_contract_dialogue(
                            player_id,
                            npc_id,
                            crate::resource_contracts::ResourceContractKind::Mining,
                        )
                        .await;
                    }
                    "board_woodcutting" => {
                        self.show_adventure_board_contract_dialogue(
                            player_id,
                            npc_id,
                            crate::resource_contracts::ResourceContractKind::Woodcutting,
                        )
                        .await;
                    }
                    "board_fishing" => {
                        self.show_adventure_board_contract_dialogue(
                            player_id,
                            npc_id,
                            crate::resource_contracts::ResourceContractKind::Fishing,
                        )
                        .await;
                    }
                    "board_smithing" => {
                        self.show_adventure_board_contract_dialogue(
                            player_id,
                            npc_id,
                            crate::resource_contracts::ResourceContractKind::Smithing,
                        )
                        .await;
                    }
                    _ => {
                        self.show_adventure_board_dialogue(player_id, npc_id).await;
                    }
                }
            }
            return;
        }

        if let Some(rest) = quest_id.strip_prefix("adventure_board_contract:") {
            if let Some((npc_id, kind_str)) = rest.split_once(':') {
                if crate::resource_contracts::ResourceContractKind::from_str(kind_str).is_some() {
                    if let Some(diff_str) = choice_id.strip_prefix("accept_") {
                        self.send_to_player(player_id, ServerMessage::DialogueClosed)
                            .await;
                        self.handle_accept_adventure_board_contract(
                            player_id, npc_id, kind_str, diff_str,
                        )
                        .await;
                    } else if choice_id == "claim_contract" {
                        self.send_to_player(player_id, ServerMessage::DialogueClosed)
                            .await;
                        self.handle_claim_resource_contract(player_id).await;
                    } else if choice_id == "abandon_contract" {
                        self.send_to_player(player_id, ServerMessage::DialogueClosed)
                            .await;
                        self.handle_abandon_resource_contract(player_id).await;
                    } else if choice_id == "nevermind" {
                        self.show_adventure_board_dialogue(player_id, npc_id).await;
                    } else {
                        self.send_to_player(player_id, ServerMessage::DialogueClosed)
                            .await;
                    }
                } else {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                }
                return;
            }
        }

        // Handle plot seller dialogue choices (format: "plot_seller:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("plot_seller:") {
            if choice_id == "buy_plots" {
                // Show the plot purchase screen
                self.show_plot_purchase_dialogue(player_id, npc_id).await;
            } else if choice_id == "contracts" {
                self.show_resource_contract_dialogue(player_id, npc_id, "master_farmer")
                    .await;
            } else if let Some(diff_str) = choice_id.strip_prefix("accept_") {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_accept_resource_contract(player_id, npc_id, "master_farmer", diff_str)
                    .await;
            } else if choice_id == "claim_contract" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_claim_resource_contract(player_id).await;
            } else if choice_id == "abandon_contract" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_abandon_resource_contract(player_id).await;
            } else if let Some(plot_str) = choice_id.strip_prefix("unlock_") {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                if let Ok(plot_id) = plot_str.parse::<u32>() {
                    self.handle_plot_purchase(player_id, plot_id).await;
                }
            } else if choice_id == "nevermind" {
                // Go back to main master farmer dialogue
                self.show_master_farmer_dialogue(player_id, npc_id).await;
            } else {
                // "close", "owned_N", "locked_N" just close
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        if let Some(rest) = quest_id.strip_prefix("resource_contract_master:") {
            if let Some((npc_id, entity_type)) = rest.split_once(':') {
                if choice_id == "contracts" {
                    self.show_resource_contract_dialogue(player_id, npc_id, entity_type)
                        .await;
                } else if choice_id == "open_shop" {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                    let _ = self
                        .try_open_merchant_shop(player_id, npc_id, entity_type)
                        .await;
                } else if let Some(diff_str) = choice_id.strip_prefix("accept_") {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                    self.handle_accept_resource_contract(player_id, npc_id, entity_type, diff_str)
                        .await;
                } else if choice_id == "claim_contract" {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                    self.handle_claim_resource_contract(player_id).await;
                } else if choice_id == "abandon_contract" {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                    self.handle_abandon_resource_contract(player_id).await;
                } else if choice_id == "nevermind" {
                    self.show_resource_contract_master_dialogue(player_id, npc_id, entity_type)
                        .await;
                } else {
                    self.send_to_player(player_id, ServerMessage::DialogueClosed)
                        .await;
                }
                return;
            }
        }

        // Handle banker dialogue choices (format: "banker:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("banker:") {
            if choice_id == "open_bank" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.handle_bank_open(player_id).await;
            } else if choice_id == "upgrade" {
                self.handle_bank_upgrade(player_id, npc_id).await;
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle KOTH rewards dialogue choices (format: "koth_rewards:{npc_id}")
        if quest_id.starts_with("koth_rewards:") {
            if choice_id == "claim" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.claim_koth_rewards(player_id).await;
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle boss rewards dialogue choices (format: "boss_rewards:{npc_id}")
        if quest_id.starts_with("boss_rewards:") {
            if choice_id == "claim" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.claim_boss_rewards(player_id).await;
            } else if choice_id == "bank" {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
                self.claim_boss_rewards_to_bank(player_id).await;
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        // Handle waystone dialogue choices (format: "waystone:{waystone_id}")
        if let Some(waystone_id) = quest_id.strip_prefix("waystone:") {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            if choice_id == "teleport" {
                self.teleport_to_waystone(player_id, waystone_id).await;
            }
            return;
        }

        // Handle port master travel choices (format: "port:{npc_id}")
        if let Some(npc_id) = quest_id.strip_prefix("port:") {
            if let Some(dest_str) = choice_id.strip_prefix("port_dest_") {
                if let Ok(dest_index) = dest_str.parse::<usize>() {
                    self.handle_port_travel(player_id, npc_id, dest_index).await;
                }
            } else {
                self.send_to_player(player_id, ServerMessage::DialogueClosed)
                    .await;
            }
            return;
        }

        self.handle_quest_dialogue_choice(player_id, quest_id, choice_id)
            .await;
    }

    async fn show_port_master_dialogue(&self, player_id: &str, npc_id: &str, entity_type: &str) {
        let prototype = match self.entity_registry.get(entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let player_gold = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => p.inventory.gold,
                None => return,
            }
        };

        let speaker = prototype.display_name.clone();
        let greeting = prototype
            .dialogue
            .greeting
            .clone()
            .unwrap_or_else(|| "Where would you like to travel?".to_string());

        let mut choices: Vec<crate::protocol::DialogueChoice> = port_config
            .destinations
            .iter()
            .enumerate()
            .map(|(i, dest)| {
                let affordable = player_gold >= dest.cost;
                let label = if affordable {
                    format!("{} - {}g", dest.name, dest.cost)
                } else {
                    format!("{} - {}g (not enough gold)", dest.name, dest.cost)
                };
                crate::protocol::DialogueChoice {
                    id: format!("port_dest_{}", i),
                    text: label,
                }
            })
            .collect();

        choices.push(crate::protocol::DialogueChoice {
            id: "close".to_string(),
            text: "Nevermind".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("port:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker,
                text: greeting,
                choices,
            },
        )
        .await;
    }

    async fn handle_port_travel(&self, player_id: &str, npc_id: &str, dest_index: usize) {
        // Close dialogue first
        self.send_to_player(player_id, ServerMessage::DialogueClosed)
            .await;

        // Look up the NPC's prototype to get the port config
        let (entity_type, npc_x, npc_y) = {
            let npcs = self.npcs.read().await;
            match npcs.get(npc_id) {
                Some(npc) => (npc.prototype_id.clone(), npc.x, npc.y),
                None => return,
            }
        };

        let prototype = match self.entity_registry.get(&entity_type) {
            Some(p) => p,
            None => return,
        };

        let port_config = match &prototype.port {
            Some(c) => c,
            None => return,
        };

        let destination = match port_config.destinations.get(dest_index) {
            Some(d) => d,
            None => return,
        };

        // Verify player is still near the NPC
        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.active && !p.is_dead => p,
            _ => return,
        };

        let dx = (player.x - npc_x) as f32;
        let dy = (player.y - npc_y) as f32;
        if (dx * dx + dy * dy).sqrt() > 5.0 {
            return;
        }

        // Check gold
        if player.inventory.gold < destination.cost {
            drop(players);
            self.send_system_message(player_id, "You don't have enough gold for that trip.")
                .await;
            return;
        }

        // Deduct gold and teleport
        player.inventory.gold -= destination.cost;
        player.x = destination.x;
        player.y = destination.y;
        // Reset movement state
        player.last_move_vel_x = 0;
        player.last_move_vel_y = 0;
        player.move_dx = 0;
        player.move_dy = 0;

        let new_gold = player.inventory.gold;
        let slots = player.inventory.to_update();
        drop(players);

        // Send inventory update with new gold
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots,
                gold: new_gold,
            },
        )
        .await;

        // Send system message
        self.send_system_message(
            player_id,
            &format!(
                "You travel to {}. (-{}g)",
                destination.name, destination.cost
            ),
        )
        .await;
    }
}
