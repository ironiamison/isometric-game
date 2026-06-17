use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "slayerPanelOpen" => {
            if let Some(value) = data {
                state.ui_state.slayer_master_id = extract_string(value, "master_id");
                state.ui_state.slayer_master_name = extract_string(value, "master_name");
                state.ui_state.slayer_current_task = extract_slayer_task(value, "current_task");
                state.ui_state.slayer_points = extract_i32(value, "points").unwrap_or(0);
                state.ui_state.slayer_tasks_completed =
                    extract_i32(value, "tasks_completed").unwrap_or(0);
                state.ui_state.slayer_rewards = extract_slayer_rewards(value, "rewards");
                state.ui_state.slayer_blocked_monsters =
                    extract_string_array(value, "blocked_monsters");
                state.ui_state.slayer_unlocked_monsters =
                    extract_string_array(value, "unlocked_monsters");
                state.ui_state.slayer_blockable_monsters =
                    extract_blockable_monsters(value, "blockable_monsters");
                state.ui_state.slayer_selected_block_monster = None;
                state.ui_state.slayer_panel_open = true;
                state.ui_state.slayer_reward_tab = 0;
                state.ui_state.slayer_reward_scroll = 0.0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "slayerTaskProgress" => {
            if let Some(value) = data {
                if let Some(ref mut task) = state.ui_state.slayer_current_task {
                    if let Some(kills) = extract_i32(value, "kills_current") {
                        task.kills_current = kills;
                    }
                    if let Some(kills) = extract_i32(value, "kills_required") {
                        task.kills_required = kills;
                    }
                }
            }
        }
        "slayerTaskComplete" => {
            if let Some(value) = data {
                let display_name = extract_string(value, "display_name").unwrap_or_default();
                let points_awarded = extract_i32(value, "points_awarded").unwrap_or(0);
                let total_points = extract_i32(value, "total_points").unwrap_or(0);
                state.ui_state.slayer_current_task = None;
                state.ui_state.slayer_points = total_points;
                // Add a system message about task completion
                state.push_system_chat(format!(
                    "Slayer task complete! {} - earned {} points (total: {}).",
                    display_name, points_awarded, total_points
                ));
            }
        }
        "slayerResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let message = extract_string(value, "message");
                if let Some(task) = extract_slayer_task(value, "task") {
                    state.ui_state.slayer_current_task = Some(task);
                }
                if let Some(points) = extract_i32(value, "points") {
                    state.ui_state.slayer_points = points;
                }
                // Clear block selection on successful block purchase
                if success {
                    state.ui_state.slayer_selected_block_monster = None;
                }
                // Show message in chat
                if let Some(msg) = message {
                    state.push_system_chat(msg);
                }
            }
        }
        "slayerStateSync" => {
            if let Some(value) = data {
                state.ui_state.slayer_current_task = extract_slayer_task(value, "current_task");
                state.ui_state.slayer_points = extract_i32(value, "points").unwrap_or(0);
                state.ui_state.slayer_tasks_completed =
                    extract_i32(value, "tasks_completed").unwrap_or(0);
                state.ui_state.slayer_blocked_monsters =
                    extract_string_array(value, "blocked_monsters");
                state.ui_state.slayer_unlocked_monsters =
                    extract_string_array(value, "unlocked_monsters");
            }
        }
        "autoActionStarted" => {
            if let Some(value) = data {
                let target_type = extract_string(value, "target_type").unwrap_or_default();
                let target_id = extract_string(value, "target_id").unwrap_or_default();
                let action = extract_string(value, "action").unwrap_or_default();
                if let Some(ref mut aa) = state.auto_action_state {
                    aa.confirmed = true;
                }
                log::debug!(
                    "Auto-action started: {} {} {}",
                    action,
                    target_type,
                    target_id
                );
            }
        }
        "autoActionStopped" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason").unwrap_or_default();
                // "cancelled" means the CLIENT sent CancelAutoAction — the client
                // already cleared old state and may have set a NEW auto-action
                // and auto-path for a different target.  Clearing here would
                // wipe the new chase, causing the "click new target → move one
                // tile → stop" bug.  Only act on server-initiated stops.
                if reason != "cancelled" {
                    if state.auto_action_state.is_some() {
                        state.auto_path = None;
                    }
                    state.auto_action_state = None;
                }
                log::debug!("Auto-action stopped: {}", reason);
            }
        }
        "autoRetaliateChanged" => {
            if let Some(value) = data {
                let enabled = extract_bool(value, "enabled").unwrap_or(true);
                state.auto_retaliate = enabled;
                log::debug!("Auto-retaliate changed: {}", enabled);
            }
        }
        "error" => {
            if let Some(value) = data {
                let message = extract_string(value, "message").unwrap_or_default();
                log::warn!("Server error: {}", message);
                state.push_system_chat(message);
                state.pending_sfx.push("error".to_string());
            }
        }

        // ===== Trade System Messages =====
        "tradeRequestReceived" => {
            if let Some(value) = data {
                let requester_id = extract_string(value, "requester_id").unwrap_or_default();
                let requester_name = extract_string(value, "requester_name").unwrap_or_default();
                state.ui_state.trade_pending_request = Some((requester_id, requester_name.clone()));
                state.push_system_chat(format!("{} wants to trade with you.", requester_name));
            }
        }
        "tradeOpened" => {
            if let Some(value) = data {
                let partner_id = extract_string(value, "partner_id").unwrap_or_default();
                let partner_name = extract_string(value, "partner_name").unwrap_or_default();
                state.ui_state.trade_open = true;
                state.ui_state.inventory_open = true;
                state.ui_state.character_panel_open = false;
                state.ui_state.stall_setup_open = false;
                state.ui_state.stall_name_editing = false;
                state.ui_state.trade_partner_id = Some(partner_id);
                state.ui_state.trade_partner_name = Some(partner_name);
                state.ui_state.trade_my_items.clear();
                state.ui_state.trade_my_gold = 0;
                state.ui_state.trade_my_accepted = false;
                state.ui_state.trade_partner_items.clear();
                state.ui_state.trade_partner_gold = 0;
                state.ui_state.trade_partner_accepted = false;
                state.ui_state.trade_pending_request = None;
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "tradeOfferUpdate" => {
            if let Some(value) = data {
                // Parse partner's items
                let mut items = Vec::new();
                if let Some(rmpv::Value::Array(list)) = extract_map_field(value, "partner_items") {
                    for item in list {
                        let slot_index = extract_u8(item, "slot_index").unwrap_or(0);
                        let item_id = extract_string(item, "item_id").unwrap_or_default();
                        let quantity = extract_i32(item, "quantity").unwrap_or(1);
                        items.push(crate::game::TradeOfferItem {
                            slot_index,
                            item_id,
                            quantity,
                        });
                    }
                }
                state.ui_state.trade_partner_items = items;
                state.ui_state.trade_partner_gold = extract_i32(value, "partner_gold").unwrap_or(0);
                state.ui_state.trade_partner_accepted =
                    extract_bool(value, "partner_accepted").unwrap_or(false);
                // If partner changed offer, reset our accepted state
                state.ui_state.trade_my_accepted = false;
            }
        }
        "tradeMyOfferUpdate" => {
            if let Some(value) = data {
                let mut items = Vec::new();
                if let Some(rmpv::Value::Array(list)) = extract_map_field(value, "my_items") {
                    for item in list {
                        let slot_index = extract_u8(item, "slot_index").unwrap_or(0);
                        let item_id = extract_string(item, "item_id").unwrap_or_default();
                        let quantity = extract_i32(item, "quantity").unwrap_or(1);
                        items.push(crate::game::TradeOfferItem {
                            slot_index,
                            item_id,
                            quantity,
                        });
                    }
                }
                state.ui_state.trade_my_items = items;
                state.ui_state.trade_my_gold = extract_i32(value, "my_gold").unwrap_or(0);
                state.ui_state.trade_my_accepted =
                    extract_bool(value, "my_accepted").unwrap_or(false);
            }
        }
        "tradeCompleted" => {
            state.ui_state.trade_open = false;
            state.ui_state.trade_partner_id = None;
            state.ui_state.trade_partner_name = None;
            state.ui_state.trade_my_items.clear();
            state.ui_state.trade_partner_items.clear();
            state.push_system_chat("Trade completed successfully!".to_string());
            state.pending_sfx.push("ui_close".to_string());
        }
        "tradeCancelled" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason")
                    .unwrap_or_else(|| "Trade cancelled.".to_string());
                state.push_system_chat(reason);
            }
            state.ui_state.trade_open = false;
            state.ui_state.trade_partner_id = None;
            state.ui_state.trade_partner_name = None;
            state.ui_state.trade_my_items.clear();
            state.ui_state.trade_partner_items.clear();
            state.ui_state.trade_pending_request = None;
            state.pending_sfx.push("ui_close".to_string());
        }

        // ===== Stall System Messages =====
        "stallOpened" => {
            if let Some(value) = data {
                let name = extract_string(value, "name").unwrap_or_default();
                state.ui_state.stall_active = true;
                state.ui_state.stall_my_name = name;
                state.ui_state.stall_setup_open = true;
                state.ui_state.inventory_open = true;
                state.push_system_chat("Your shop is now open!".to_string());
            }
        }
        "stallClosed" => {
            state.ui_state.stall_active = false;
            state.ui_state.stall_my_slots.clear();
            state.ui_state.stall_setup_open = false;
            state.push_system_chat("Your shop has been closed.".to_string());
        }
        "stallUpdate" => {
            if let Some(value) = data {
                let mut slots = Vec::new();
                if let Some(rmpv::Value::Array(list)) = extract_map_field(value, "slots") {
                    for item in list {
                        let slot = extract_u8(item, "slot").unwrap_or(0);
                        let item_id = extract_string(item, "item_id").unwrap_or_default();
                        let quantity = extract_i32(item, "quantity").unwrap_or(1);
                        let price = extract_i32(item, "price").unwrap_or(0);
                        slots.push(crate::game::StallSlotInfo {
                            slot,
                            item_id,
                            quantity,
                            price,
                        });
                    }
                }
                state.ui_state.stall_my_slots = slots;
            }
        }
        "stallBrowseData" => {
            if let Some(value) = data {
                let seller_id = extract_string(value, "seller_id").unwrap_or_default();
                let seller_name = extract_string(value, "seller_name").unwrap_or_default();
                let stall_name = extract_string(value, "stall_name").unwrap_or_default();
                let mut items = Vec::new();
                if let Some(rmpv::Value::Array(list)) = extract_map_field(value, "items") {
                    for item in list {
                        let slot = extract_u8(item, "slot").unwrap_or(0);
                        let item_id = extract_string(item, "item_id").unwrap_or_default();
                        let quantity = extract_i32(item, "quantity").unwrap_or(1);
                        let price = extract_i32(item, "price").unwrap_or(0);
                        items.push(crate::game::StallSlotInfo {
                            slot,
                            item_id,
                            quantity,
                            price,
                        });
                    }
                }
                state.ui_state.stall_browse = Some(crate::game::StallBrowseInfo {
                    seller_id,
                    seller_name,
                    stall_name,
                    items,
                });
                state.ui_state.stall_buy_quantity = 1;
                state.ui_state.stall_browse_selected = 0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "stallBuyResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                if success {
                    let item_id = extract_string(value, "item_id").unwrap_or_default();
                    let quantity = extract_i32(value, "quantity").unwrap_or(1);
                    let total_price = extract_i32(value, "total_price").unwrap_or(0);
                    let item_def = state.item_registry.get_or_placeholder(&item_id);
                    state.push_system_chat(format!(
                        "Bought {}x {} for {}g",
                        quantity, item_def.display_name, total_price
                    ));
                } else {
                    let error =
                        extract_string(value, "error").unwrap_or("Unknown error".to_string());
                    state.push_system_chat(format!("Purchase failed: {}", error));
                }
            }
        }
        "stallSaleNotification" => {
            if let Some(value) = data {
                let buyer_name = extract_string(value, "buyer_name").unwrap_or_default();
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(1);
                let gold_earned = extract_i32(value, "gold_received").unwrap_or(0);
                let item_def = state.item_registry.get_or_placeholder(&item_id);
                state.push_system_chat(format!(
                    "{} bought {}x {} from your shop for {}g!",
                    buyer_name, quantity, item_def.display_name, gold_earned
                ));
            }
        }
        "stallItemUpdate" => {
            if let Some(value) = data {
                let seller_id = extract_string(value, "seller_id").unwrap_or_default();
                let slot = extract_u8(value, "stall_slot").unwrap_or(0);
                let quantity = extract_i32(value, "new_quantity").unwrap_or(0);
                // Update browse data if open and matches the seller
                if let Some(ref mut browse) = state.ui_state.stall_browse {
                    if browse.seller_id == seller_id {
                        if let Some(item) = browse.items.iter_mut().find(|i| i.slot == slot) {
                            if quantity <= 0 {
                                browse.items.retain(|i| i.slot != slot);
                            } else {
                                item.quantity = quantity;
                            }
                        }
                    }
                }
            }
        }
        "topPlayerChanged" => {
            if let Some(value) = data {
                state.top_level_player_name = extract_string(value, "player_name");
                state.second_level_player_name = extract_string(value, "second_player_name");
            }
        }
        "kothStateUpdate" => {
            if let Some(value) = data {
                let phase = extract_string(value, "phase").unwrap_or_default();
                let wave = extract_u32(value, "wave").unwrap_or(0);
                let points = extract_u32(value, "points").unwrap_or(0);
                let enemies_alive = extract_u32(value, "enemiesAlive").unwrap_or(0);
                let enemies_total = extract_u32(value, "enemiesTotal").unwrap_or(0);
                let countdown_ms = extract_u32(value, "countdownMs").unwrap_or(0);

                state.koth = Some(crate::game::KothClientState {
                    phase,
                    wave,
                    points,
                    enemies_alive,
                    enemies_total,
                    countdown_ms,
                });
            }
        }
        "kothCheckpoint" => {
            if let Some(value) = data {
                let wave = extract_u32(value, "wave").unwrap_or(0);
                let points = extract_u32(value, "points").unwrap_or(0);
                let next_wave_enemy_count = extract_u32(value, "nextWaveEnemyCount").unwrap_or(0);

                let mut rewards = Vec::new();
                if let Some(rewards_arr) = extract_array(value, "rewards") {
                    for r in rewards_arr {
                        rewards.push(crate::game::KothRewardPreview {
                            item_id: extract_string(r, "itemId").unwrap_or_default(),
                            quantity: extract_u32(r, "quantity").unwrap_or(0),
                        });
                    }
                }

                state.koth_checkpoint_open = true;
                // Store checkpoint info in koth state
                if let Some(ref mut koth) = state.koth {
                    koth.phase = "checkpoint".to_string();
                    koth.wave = wave;
                    koth.points = points;
                }
                // Store checkpoint rewards for UI display
                state.koth_checkpoint_info = Some(crate::game::KothCheckpointInfo {
                    wave,
                    points,
                    rewards,
                    next_wave_enemy_count,
                });
            }
        }
        "kothGameOver" => {
            if let Some(value) = data {
                let waves_completed = extract_u32(value, "wavesCompleted").unwrap_or(0);
                let total_points = extract_u32(value, "totalPoints").unwrap_or(0);
                let victory = extract_bool(value, "victory").unwrap_or(false);

                let mut rewards = Vec::new();
                if let Some(rewards_arr) = extract_array(value, "rewards") {
                    for r in rewards_arr {
                        rewards.push(crate::game::KothRewardPreview {
                            item_id: extract_string(r, "itemId").unwrap_or_default(),
                            quantity: extract_u32(r, "quantity").unwrap_or(0),
                        });
                    }
                }

                state.koth_game_over = Some(crate::game::KothGameOverInfo {
                    waves_completed,
                    total_points,
                    rewards,
                    victory,
                    shown_at: macroquad::prelude::get_time(),
                });
                state.koth = None; // Clear HUD state
                state.koth_checkpoint_open = false;
            }
        }
        "bossStateUpdate" => {
            if let Some(value) = data {
                let boss_id = extract_string(value, "bossId").unwrap_or_default();
                let hp = extract_i32(value, "hp").unwrap_or(0);
                let max_hp = extract_i32(value, "maxHp").unwrap_or(0);
                let phase = extract_string(value, "phase").unwrap_or_default();
                let wurm_state = extract_string(value, "wurmState").unwrap_or_default();

                if phase == "dead" {
                    state.boss = None;
                    state.reaper_mark = None;
                } else {
                    state.boss = Some(crate::game::state::BossClientState {
                        boss_id,
                        hp,
                        max_hp,
                        phase,
                        wurm_state,
                    });
                }
            }
        }
        "aoeWarning" => {
            if let Some(value) = data {
                let delay_ms = extract_u64(value, "delayMs").unwrap_or(1000);
                let effect = extract_string(value, "effect").unwrap_or_default();

                let mut tiles = Vec::new();
                if let Some(tiles_arr) = extract_array(value, "tiles") {
                    for tile_val in tiles_arr {
                        if let Some(arr) = tile_val.as_array() {
                            if arr.len() >= 2 {
                                let tx = arr[0].as_i64().unwrap_or(0) as i32;
                                let ty = arr[1].as_i64().unwrap_or(0) as i32;
                                tiles.push((tx, ty));
                            }
                        }
                    }
                }

                state.aoe_warnings.push(crate::game::state::AoeWarningZone {
                    tiles,
                    created_at: current_time(),
                    delay_ms,
                    effect,
                });
            }
        }
        "aoeDamage" => {
            if let Some(value) = data {
                let mut tiles = Vec::new();
                if let Some(tiles_arr) = extract_array(value, "tiles") {
                    for tile_val in tiles_arr {
                        if let Some(arr) = tile_val.as_array() {
                            if arr.len() >= 2 {
                                let tx = arr[0].as_i64().unwrap_or(0) as i32;
                                let ty = arr[1].as_i64().unwrap_or(0) as i32;
                                tiles.push((tx, ty));
                            }
                        }
                    }
                }

                let effect = extract_string(value, "effect").unwrap_or_default();

                if !effect.is_empty() {
                    if effect == "rocks_aoe" {
                        state.pending_sfx.push("aoe_rockfall".to_string());
                    }
                    for (tx, ty) in &tiles {
                        state.spell_effects.push(SpellEffect {
                            caster_id: String::new(),
                            target_id: None,
                            spell_id: effect.clone(),
                            target_x: *tx,
                            target_y: *ty,
                            time: current_time(),
                        });
                    }
                } else {
                    // Legacy fallback: orange diamond overlay
                    for (tx, ty) in tiles {
                        state.explosions.push(crate::game::state::ExplosionEffect {
                            x: tx,
                            y: ty,
                            radius: 0,
                            created_at: current_time(),
                        });
                    }
                }
            }
        }
        "explosion" => {
            if let Some(value) = data {
                let x = extract_i32(value, "x").unwrap_or(0);
                let y = extract_i32(value, "y").unwrap_or(0);
                let radius = extract_i32(value, "radius").unwrap_or(3);

                state.explosions.push(crate::game::state::ExplosionEffect {
                    x,
                    y,
                    radius,
                    created_at: current_time(),
                });
                state.pending_sfx.push("rock_explode".to_string());
            }
        }
        "reaperMark" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "playerId").unwrap_or_default();
                let duration_ms = extract_u64(value, "durationMs").unwrap_or(0);
                if duration_ms == 0 {
                    // Clear only if this player's mark was the active one.
                    if state
                        .reaper_mark
                        .as_ref()
                        .map(|m| m.player_id == player_id)
                        .unwrap_or(false)
                    {
                        state.reaper_mark = None;
                    }
                } else {
                    if state.local_player_id.as_deref() == Some(player_id.as_str()) {
                        state.pending_sfx.push("error".to_string());
                    }
                    state.reaper_mark = Some(crate::game::state::ReaperMarkState {
                        player_id,
                        created_at: current_time(),
                        duration_ms,
                    });
                }
            }
        }

        _ => return false,
    }
    true
}
