use super::*;

#[derive(Debug)]
pub(super) enum ClientMessageError {
    Decode(String),
    SessionSuperseded,
}

impl std::fmt::Display for ClientMessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "{error}"),
            Self::SessionSuperseded => write!(f, "session is no longer active"),
        }
    }
}

pub(super) async fn handle_client_message(
    state: &AppState,
    room: &GameRoom,
    session_id: &str,
    player_id: &str,
    data: &[u8],
) -> Result<(), ClientMessageError> {
    // Keep the command gate read-locked through execution. Session takeover and
    // logout wait for in-flight commands, while later stale commands are rejected.
    let _session_lease = acquire_session_lease(
        &state.sessions,
        &state.auth_sessions,
        session_id,
        &room.id,
        player_id,
    )
    .await
    .ok_or(ClientMessageError::SessionSuperseded)?;

    let msg = protocol::decode_client_message(data).map_err(ClientMessageError::Decode)?;
    let handler_start = std::time::Instant::now();
    let msg_name = msg.name();

    match msg {
        ClientMessage::Move { dx, dy, seq } => {
            room.handle_move(player_id, dx, dy, seq).await;
        }
        ClientMessage::Dash => {
            room.handle_dash(player_id).await;
        }
        ClientMessage::Jump => {
            room.handle_jump(player_id).await;
        }
        ClientMessage::Face { direction } => {
            room.handle_face(player_id, direction).await;
        }
        ClientMessage::Chat { text, channel } => {
            room.handle_chat(player_id, &text, &channel).await;
        }
        ClientMessage::Attack => {
            room.handle_attack(player_id, None, None).await;
        }
        ClientMessage::Target { entity_id } => {
            room.handle_target(player_id, &entity_id).await;
        }
        ClientMessage::Pickup { item_id } => {
            room.handle_pickup(player_id, &item_id).await;
        }
        ClientMessage::UseItem { slot_index } => {
            room.handle_use_item(player_id, slot_index).await;
        }
        ClientMessage::RequestChunk { chunk_x, chunk_y } => {
            if let Some(chunk_msg) = room.handle_chunk_request(chunk_x, chunk_y).await {
                room.send_to_player(player_id, chunk_msg).await;
            }
        }
        ClientMessage::Interact { npc_id } => {
            room.handle_npc_interact(player_id, &npc_id).await;
        }
        ClientMessage::InteractObject { x, y } => {
            room.handle_interact_object(player_id, x, y).await;
        }
        ClientMessage::UseWaystone { x, y } => {
            room.handle_use_waystone(player_id, x, y).await;
        }
        ClientMessage::DialogueChoiceMsg {
            quest_id,
            choice_id,
        } => {
            room.handle_dialogue_choice(player_id, &quest_id, &choice_id)
                .await;
        }
        ClientMessage::AcceptQuest { quest_id: _ } => {
            // Quest acceptance is handled through dialogue choices
            // This is a fallback if client sends direct accept
        }
        ClientMessage::AbandonQuest { quest_id: _ } => {
            // TODO: Implement quest abandonment
        }
        ClientMessage::Craft { recipe_id } => {
            room.handle_craft(player_id, &recipe_id).await;
        }
        ClientMessage::StartCraft { recipe_id } => {
            room.handle_start_craft(player_id, &recipe_id).await;
        }
        ClientMessage::CancelCraft => {
            room.handle_cancel_craft(player_id).await;
        }
        ClientMessage::Equip { slot_index } => {
            room.handle_equip(player_id, slot_index).await;
        }
        ClientMessage::Unequip { slot_type, .. } => {
            room.handle_unequip(player_id, &slot_type).await;
        }
        ClientMessage::DropItem {
            slot_index,
            quantity,
            target_x,
            target_y,
        } => {
            room.handle_drop_item(player_id, slot_index, quantity, target_x, target_y)
                .await;
        }
        ClientMessage::DropGold { amount } => {
            room.handle_drop_gold(player_id, amount).await;
        }
        ClientMessage::SwapSlots { from_slot, to_slot } => {
            room.handle_swap_slots(player_id, from_slot, to_slot).await;
        }
        ClientMessage::ShopBuy {
            npc_id,
            item_id,
            quantity,
        } => {
            room.handle_shop_buy(player_id, &npc_id, &item_id, quantity)
                .await;
        }
        ClientMessage::ShopSell {
            npc_id,
            item_id,
            quantity,
        } => {
            room.handle_shop_sell(player_id, &npc_id, &item_id, quantity)
                .await;
        }
        ClientMessage::EnterPortal { portal_id } => {
            handle_enter_portal(state, room, player_id, &portal_id).await;
        }
        ClientMessage::StartGathering { marker_x, marker_y } => {
            room.handle_start_gathering(player_id, marker_x, marker_y)
                .await;
        }
        ClientMessage::StopGathering => {
            room.handle_stop_gathering(player_id).await;
        }
        ClientMessage::ChopTree {
            tree_x,
            tree_y,
            tree_gid,
        } => {
            room.handle_chop_tree(player_id, tree_x, tree_y, tree_gid)
                .await;
        }
        ClientMessage::MineRock {
            rock_x,
            rock_y,
            rock_gid,
        } => {
            room.handle_mine_rock(player_id, rock_x, rock_y, rock_gid)
                .await;
        }
        ClientMessage::SitChair { tile_x, tile_y } => {
            room.handle_sit_chair(player_id, tile_x, tile_y).await;
        }
        ClientMessage::StandUp => {
            room.handle_stand_up(player_id).await;
        }
        ClientMessage::PlantSeed { patch_id, item_id } => {
            room.handle_plant_seed(player_id, &patch_id, &item_id).await;
        }
        ClientMessage::HarvestCrop { patch_id } => {
            room.handle_harvest_crop(player_id, &patch_id).await;
        }
        // Friend system messages
        ClientMessage::SendFriendRequest { target_name } => {
            room.handle_send_friend_request(player_id, &target_name)
                .await;
        }
        ClientMessage::AcceptFriendRequest { requester_id } => {
            room.handle_accept_friend_request(player_id, requester_id)
                .await;
        }
        ClientMessage::DeclineFriendRequest { requester_id } => {
            room.handle_decline_friend_request(player_id, requester_id)
                .await;
        }
        ClientMessage::RemoveFriend { friend_id } => {
            room.handle_remove_friend(player_id, friend_id).await;
        }
        ClientMessage::GetOnlinePlayers => {
            room.handle_get_online_players(player_id).await;
        }
        // Prayer system messages
        ClientMessage::TogglePrayer { prayer_id } => {
            room.handle_toggle_prayer(player_id, &prayer_id).await;
        }
        ClientMessage::BuryBones { slot } => {
            room.handle_bury_bones(player_id, slot).await;
        }
        ClientMessage::OfferBones { slot, altar_id } => {
            room.handle_offer_bones(player_id, slot, &altar_id).await;
        }
        ClientMessage::OfferAllBones { item_id, altar_id } => {
            room.handle_offer_all_bones(player_id, &item_id, &altar_id)
                .await;
        }
        ClientMessage::PrayAtAltar { altar_id } => {
            room.handle_pray_at_altar(player_id, &altar_id).await;
        }
        // Spell system messages
        ClientMessage::CastSpell { spell_id } => {
            if spell_id == "return_home" {
                // Return Home needs special instance cleanup handling
                use crate::interior::InstanceType;

                let spell_def = match crate::spell::get_spell(&spell_id) {
                    Some(s) => s,
                    None => {
                        room.handle_cast_spell(player_id, &spell_id).await;
                        return Ok(());
                    }
                };
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let instance_id: Option<String> = {
                    let instances = state.player_instances.read().await;
                    instances.get(player_id).cloned()
                };

                let success = room
                    .cast_return_home_spell(player_id, spell_def, current_time)
                    .await;

                if success {
                    // Stop gathering (fishing, etc.) on teleport
                    room.handle_stop_gathering(player_id).await;

                    if let Some(instance_id) = instance_id {
                        // Player was in an instance - do instance cleanup
                        {
                            let mut instances = state.player_instances.write().await;
                            instances.remove(player_id);
                        }

                        if let Some(instance) =
                            state.instance_manager.get_by_instance_id(&instance_id)
                        {
                            let other_players: Vec<String> = instance
                                .get_player_ids()
                                .await
                                .into_iter()
                                .filter(|id| id != player_id)
                                .collect();

                            let remaining = instance.remove_player(player_id).await;
                            if remaining == 0
                                && instance.instance_type == InstanceType::Private
                                && let Some(owner_id) = &instance.owner_id
                            {
                                state
                                    .instance_manager
                                    .remove_private(owner_id, &instance.map_id);
                            }

                            for other_id in &other_players {
                                room.send_to_player(
                                    other_id,
                                    ServerMessage::PlayerLeft {
                                        id: player_id.to_string(),
                                    },
                                )
                                .await;
                                room.send_to_player(
                                    player_id,
                                    ServerMessage::PlayerLeft {
                                        id: other_id.clone(),
                                    },
                                )
                                .await;
                            }
                        }

                        // Clean up entrance position
                        {
                            let mut entrance_positions =
                                state.player_entrance_positions.write().await;
                            entrance_positions.remove(player_id);
                        }

                        // Send map transition to overworld
                        let spawn_x = -30.0_f32;
                        let spawn_y = 19.0_f32;
                        let spawn_chunk = chunk::ChunkCoord::from_world(
                            spawn_x.floor() as i32,
                            spawn_y.floor() as i32,
                        );
                        room.world()
                            .preload_chunks(spawn_chunk, game::SPAWN_PRELOAD_RADIUS)
                            .await;
                        room.send_to_player(
                            player_id,
                            ServerMessage::MapTransition {
                                map_type: "overworld".to_string(),
                                map_id: "world_0".to_string(),
                                spawn_x,
                                spawn_y,
                                instance_id: String::new(),
                            },
                        )
                        .await;

                        // Re-send overworld data
                        room.send_to_player(player_id, room.get_chair_positions_message().await)
                            .await;
                        room.send_to_player(
                            player_id,
                            room.get_gathering_markers_message(None).await,
                        )
                        .await;
                        room.send_to_player(
                            player_id,
                            room.get_chest_positions_message(None).await,
                        )
                        .await;

                        // Notify overworld players
                        {
                            let player_name =
                                room.get_player_name(player_id).await.unwrap_or_default();
                            let (gender, skin) = room
                                .get_player_appearance(player_id)
                                .await
                                .unwrap_or_else(|| ("male".to_string(), "tan".to_string()));
                            let (hair_style, hair_color) = room
                                .get_player_hair(player_id)
                                .await
                                .unwrap_or((None, None));
                            room.send_to_overworld_players(
                                ServerMessage::PlayerJoined {
                                    id: player_id.to_string(),
                                    name: player_name,
                                    x: spawn_x as i32,
                                    y: spawn_y as i32,
                                    gender,
                                    skin,
                                    hair_style,
                                    hair_color,
                                },
                                Some(player_id),
                            )
                            .await;
                        }
                    }
                }
            } else {
                room.handle_cast_spell(player_id, &spell_id).await;
            }
        }
        // Auth and Register are handled via HTTP endpoints, not WebSocket
        // ===== Bank Messages =====
        ClientMessage::BankDeposit { item_id, quantity } => {
            room.handle_bank_deposit(player_id, &item_id, quantity)
                .await;
        }
        ClientMessage::BankWithdraw { item_id, quantity } => {
            room.handle_bank_withdraw(player_id, &item_id, quantity)
                .await;
        }
        ClientMessage::BankDepositGold { amount } => {
            room.handle_bank_deposit_gold(player_id, amount).await;
        }
        ClientMessage::BankWithdrawGold { amount } => {
            room.handle_bank_withdraw_gold(player_id, amount).await;
        }
        ClientMessage::BankDepositAll => {
            room.handle_bank_deposit_all(player_id).await;
        }
        ClientMessage::BankSwapSlots { slot_a, slot_b } => {
            room.handle_bank_swap_slots(player_id, slot_a, slot_b).await;
        }
        ClientMessage::BankSort => {
            room.handle_bank_sort(player_id).await;
        }

        ClientMessage::Auth { .. } | ClientMessage::Register { .. } => {}
        ClientMessage::StartCraftBatch {
            recipe_id,
            quantity,
        } => {
            room.handle_start_craft_batch(player_id, &recipe_id, quantity)
                .await;
        }
        // Ping/Pong for latency measurement
        ClientMessage::Ping { timestamp } => {
            room.send_to_player(player_id, ServerMessage::Pong { timestamp })
                .await;
        }
        ClientMessage::SlayerGetTask { master_id } => {
            room.handle_slayer_get_task(player_id, &master_id).await;
        }
        ClientMessage::SlayerCancelTask => {
            room.handle_slayer_cancel_task(player_id).await;
        }
        ClientMessage::SlayerBuyReward {
            reward_id,
            target_monster_id,
        } => {
            room.handle_slayer_buy_reward(player_id, &reward_id, target_monster_id)
                .await;
        }
        ClientMessage::SlayerRemoveBlock { monster_id } => {
            room.handle_slayer_remove_block(player_id, &monster_id)
                .await;
        }
        ClientMessage::StartAutoAction {
            target_type,
            target_id,
            action,
        } => {
            room.handle_start_auto_action(player_id, &target_type, &target_id, &action)
                .await;
        }
        ClientMessage::CancelAutoAction => {
            room.handle_cancel_auto_action(player_id).await;
        }
        ClientMessage::SetAutoRetaliate { enabled } => {
            room.handle_set_auto_retaliate(player_id, enabled).await;
        }
        // ===== Chest System Messages =====
        ClientMessage::OpenChest { x, y } => {
            room.handle_open_chest(player_id, x, y).await;
        }
        ClientMessage::ChestTake { chest_id, slot } => {
            room.handle_chest_take(player_id, &chest_id, slot).await;
        }
        ClientMessage::ChestDeposit {
            chest_id,
            inventory_slot,
        } => {
            room.handle_chest_deposit(player_id, &chest_id, inventory_slot)
                .await;
        }
        ClientMessage::SpectatorUpgrade { .. } => {
            // Handled by spectator WebSocket handler, not the normal game message dispatch
            tracing::warn!(
                "SpectatorUpgrade received in normal message handler for player {}",
                player_id
            );
        }
        // Trade system messages
        ClientMessage::TradeRequest { target_id } => {
            room.handle_trade_request(player_id, &target_id).await;
        }
        ClientMessage::TradeAcceptRequest { requester_id } => {
            room.handle_trade_accept_request(player_id, &requester_id)
                .await;
        }
        ClientMessage::TradeDeclineRequest { requester_id } => {
            room.handle_trade_decline_request(player_id, &requester_id)
                .await;
        }
        ClientMessage::TradeOfferItem {
            slot_index,
            quantity,
        } => {
            room.handle_trade_offer_item(player_id, slot_index, quantity)
                .await;
        }
        ClientMessage::TradeRemoveItem { offer_index } => {
            room.handle_trade_remove_item(player_id, offer_index).await;
        }
        ClientMessage::TradeOfferGold { amount } => {
            room.handle_trade_offer_gold(player_id, amount).await;
        }
        ClientMessage::TradeAccept => {
            room.handle_trade_accept(player_id).await;
        }
        ClientMessage::TradeCancel => {
            room.handle_trade_cancel(player_id).await;
        }
        // Stall system messages
        ClientMessage::StallOpen { name } => {
            room.handle_stall_open(player_id, &name).await;
        }
        ClientMessage::StallClose => {
            room.handle_stall_close(player_id).await;
        }
        ClientMessage::StallSetItem {
            inventory_slot,
            quantity,
            price,
        } => {
            room.handle_stall_set_item(player_id, inventory_slot, quantity, price)
                .await;
        }
        ClientMessage::StallRemoveItem { stall_slot } => {
            room.handle_stall_remove_item(player_id, stall_slot).await;
        }
        ClientMessage::StallUpdatePrice { stall_slot, price } => {
            room.handle_stall_update_price(player_id, stall_slot, price)
                .await;
        }
        ClientMessage::StallBrowse {
            player_id: target_id,
        } => {
            room.handle_stall_browse(player_id, &target_id).await;
        }
        ClientMessage::StallBuy {
            seller_id,
            stall_slot,
            quantity,
            expected_price,
        } => {
            room.handle_stall_buy(player_id, &seller_id, stall_slot, quantity, expected_price)
                .await;
        }
        ClientMessage::SetCombatStyle { style } => {
            if let Some(combat_style) = crate::game::CombatStyle::from_str(&style) {
                room.set_combat_style(player_id, combat_style).await;
            } else {
                tracing::warn!("Player {} sent invalid combat style: {}", player_id, style);
            }
        }
        ClientMessage::KothContinue => {
            let ct = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            room.handle_koth_continue(player_id, ct).await;
        }
        ClientMessage::KothLeave => {
            let ct = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            room.handle_koth_leave(player_id, ct).await;
        }
        ClientMessage::UseItemOn {
            slot_index,
            target_npc_id,
        } => {
            room.handle_use_item_on(player_id, slot_index, &target_npc_id)
                .await;
        }
        ClientMessage::GeOpen => {
            room.handle_ge_open(player_id).await;
        }
        ClientMessage::GePlaceOffer {
            side,
            item_id,
            price,
            quantity,
        } => {
            room.handle_ge_place_offer(player_id, &side, &item_id, price, quantity)
                .await;
        }
        ClientMessage::GeCancelOffer { offer_id } => {
            room.handle_ge_cancel_offer(player_id, offer_id).await;
        }
        ClientMessage::GeCollect { offer_id } => {
            room.handle_ge_collect(player_id, offer_id).await;
        }
    }

    let handler_duration = handler_start.elapsed();
    let handler_ms = handler_duration.as_secs_f64() * 1000.0;
    state.perf_metrics.record_handler(msg_name, handler_ms);
    if handler_duration.as_millis() > 20 {
        tracing::warn!(
            "Slow handler: {} took {:.2}ms for player {}",
            msg_name,
            handler_ms,
            player_id
        );
    }

    Ok(())
}
