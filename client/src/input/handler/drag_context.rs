use super::*;

impl InputHandler {
    pub(super) fn handle_context_menu(
        &mut self,
        state: &mut GameState,
        frame: ProcessFrame<'_>,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let mx = frame.mx;
        let my = frame.my;
        let mouse_clicked = frame.mouse_clicked;
        let mouse_right_clicked = frame.mouse_right_clicked;
        let clicked_element = frame.clicked_element.clone();
        // Handle context menu interactions first
        if let Some(ref menu) = state.ui_state.context_menu {
            // Auto-hide context menu when mouse leaves its bounds
            // Use generous estimates — exact size depends on text measurement in renderer,
            // but we just need a rough bounding box to dismiss when mouse wanders far away.
            let option_height = 20.0;
            let num_options = match &menu.target {
                ContextMenuTarget::EquipmentSlot(_) => 1,
                ContextMenuTarget::Gold => 1,
                ContextMenuTarget::InventorySlot(slot_index) => {
                    let (is_equippable, is_bones, is_knife) = state
                        .inventory
                        .slots
                        .get(*slot_index)
                        .and_then(|s| s.as_ref())
                        .map(|slot| {
                            let item_def = state.item_registry.get_or_placeholder(&slot.item_id);
                            let equippable = item_def.equipment.is_some();
                            let bones = slot.item_id.contains("bones");
                            let knife = slot.item_id == "knife";
                            (equippable, bones, knife)
                        })
                        .unwrap_or((false, false, false));
                    let has_deposit = state.ui_state.chest_open;
                    1 + if is_equippable { 1 } else { 0 }
                        + if is_bones { 1 } else { 0 }
                        + if is_knife { 1 } else { 0 }
                        + if has_deposit { 1 } else { 0 }
                        + if cfg!(target_os = "android") { 3 } else { 0 } // Hotkey 1-3
                }
                ContextMenuTarget::Player { .. } => 4,
                ContextMenuTarget::Npc { id } => state
                    .npcs
                    .get(id)
                    .map(|npc| {
                        if npc.is_attackable() {
                            3
                        } else if npc.is_altar {
                            3
                        } else if npc.is_merchant {
                            3
                        } else {
                            2
                        }
                    })
                    .unwrap_or(1),
                ContextMenuTarget::Tree { .. } => 2,
                ContextMenuTarget::Rock { .. } => 2,
                ContextMenuTarget::MapObject { .. } => 2,
                ContextMenuTarget::GatheringSpot { .. } => 2,
                ContextMenuTarget::GroundItem { .. } => 2,
                ContextMenuTarget::FarmingPatch { patch_id } => state
                    .farming_patches
                    .get(patch_id)
                    .map(|p| {
                        if p.state == "harvestable" || p.state == "empty" {
                            2
                        } else {
                            1
                        }
                    })
                    .unwrap_or(1),
                ContextMenuTarget::Tile { .. } => 1,
                ContextMenuTarget::HotkeySlot(_) => 1, // "Clear Slot"
                ContextMenuTarget::Spell(_) => 3,      // "Hotkey 1", "Hotkey 2", "Hotkey 3"
                ContextMenuTarget::QuestTracker => 1,  // "Minimize" or "Expand"
                ContextMenuTarget::ChatTab => 1,       // "Hide/Show System Messages"
                ContextMenuTarget::BankSlot(idx) => state
                    .ui_state
                    .bank_slots
                    .get(*idx)
                    .and_then(|s| s.as_ref())
                    .map(|(_, qty)| if *qty > 1 { 3 } else { 1 })
                    .unwrap_or(0),
                ContextMenuTarget::BankInventorySlot(idx) => state
                    .inventory
                    .slots
                    .get(*idx)
                    .and_then(|s| s.as_ref())
                    .map(|slot| if slot.quantity > 1 { 3 } else { 1 })
                    .unwrap_or(0),
            };

            let menu_width = 140.0; // generous estimate
            let menu_height = option_height + num_options as f32 * option_height + 4.0;

            let mut menu_x = menu.x.floor();
            let mut menu_y = menu.y.floor();
            let screen_w = screen_width();
            let screen_h = screen_height();
            if menu_x + menu_width > screen_w {
                menu_x = (screen_w - menu_width - 2.0).floor();
            }
            if menu_y + menu_height > screen_h {
                menu_y = (screen_h - menu_height - 2.0).floor();
            }

            let margin = 6.0;
            let is_mouse_inside = mx >= menu_x - margin
                && mx <= menu_x + menu_width + margin
                && my >= menu_y - margin
                && my <= menu_y + menu_height + margin;

            if !is_mouse_inside {
                state.ui_state.context_menu = None;
            }
        }

        if state.ui_state.context_menu.is_some() {
            if let Some(ref element) = clicked_element {
                if mouse_clicked {
                    match element {
                        UiElementId::ContextMenuOption(option_idx) => {
                            // Get menu info before clearing it
                            let menu = state.ui_state.context_menu.take().unwrap();

                            match &menu.target {
                                ContextMenuTarget::EquipmentSlot(slot_type) => {
                                    // Equipment slot context menu - only unequip option
                                    if *option_idx == 0 {
                                        commands.push(InputCommand::Unequip {
                                            slot_type: slot_type.clone(),
                                            target_slot: None, // Use first available slot
                                        });
                                    }
                                }
                                ContextMenuTarget::Gold => {
                                    // Gold context menu - only drop option
                                    if *option_idx == 0 {
                                        // Open gold drop dialog
                                        state.ui_state.gold_drop_dialog = Some(GoldDropDialog {
                                            input: String::new(),
                                            cursor: 0,
                                        });
                                    }
                                }
                                ContextMenuTarget::InventorySlot(slot_index) => {
                                    // Inventory slot context menu
                                    // Determine menu options based on item type
                                    let (is_equippable, is_bones, is_dig, is_knife, has_item) =
                                        state
                                            .inventory
                                            .slots
                                            .get(*slot_index)
                                            .and_then(|s| s.as_ref())
                                            .map(|slot| {
                                                let item_def = state
                                                    .item_registry
                                                    .get_or_placeholder(&slot.item_id);
                                                let equippable = item_def.equipment.is_some();
                                                let bones = slot.item_id.contains("bones");
                                                let dig =
                                                    item_def.use_effect.as_deref() == Some("dig");
                                                let knife = slot.item_id == "knife";
                                                (equippable, bones, dig, knife, true)
                                            })
                                            .unwrap_or((false, false, false, false, false));
                                    let chest_open = state.ui_state.chest_open && has_item;

                                    // Build option index mapping: [Equip?] [Bury?] [Dig?] [Fletch?] [Deposit?] Drop
                                    let mut current_idx = 0usize;
                                    let equip_idx = if is_equippable {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let bury_idx = if is_bones {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let dig_idx = if is_dig {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let fletch_idx = if is_knife {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let deposit_idx = if chest_open {
                                        let idx = current_idx;
                                        current_idx += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let drop_idx = current_idx;
                                    current_idx += 1;
                                    let hotkey_base_idx = if cfg!(target_os = "android") {
                                        Some(current_idx)
                                    } else {
                                        None
                                    };

                                    if Some(*option_idx) == equip_idx {
                                        commands.push(InputCommand::Equip {
                                            slot_index: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == bury_idx {
                                        commands.push(InputCommand::BuryBones {
                                            slot: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == dig_idx {
                                        commands.push(InputCommand::UseItem {
                                            slot_index: *slot_index as u8,
                                        });
                                    } else if Some(*option_idx) == fletch_idx {
                                        state.ui_state.fletching_open = true;
                                        state.ui_state.fletching_selected_recipe = 0;
                                        state.ui_state.fletching_scroll_offset = 0.0;
                                        state.ui_state.fletching_quantity = 1;
                                        state.ui_state.fletching_tab = 0;
                                        state.pending_sfx.push("ui_open".to_string());
                                    } else if Some(*option_idx) == deposit_idx {
                                        commands.push(InputCommand::ChestDeposit {
                                            chest_id: state.ui_state.chest_id.clone(),
                                            inventory_slot: *slot_index as u8,
                                        });
                                    } else if *option_idx == drop_idx {
                                        if let Some(slot) = state
                                            .inventory
                                            .slots
                                            .get(*slot_index)
                                            .and_then(|s| s.as_ref())
                                        {
                                            commands.push(InputCommand::DropItem {
                                                slot_index: *slot_index as u8,
                                                quantity: slot.quantity as u32,
                                                target_x: None,
                                                target_y: None,
                                            });
                                        }
                                    } else if let Some(base) = hotkey_base_idx {
                                        if *option_idx >= base && *option_idx < base + 3 {
                                            let hotkey_slot = *option_idx - base;
                                            if let Some(Some(slot)) =
                                                state.inventory.slots.get(*slot_index)
                                            {
                                                state.ui_state.hotkey_bar.active_mut().slots
                                                    [hotkey_slot] =
                                                    crate::game::hotkey::HotkeySlotBinding::Item {
                                                        item_id: slot.item_id.clone(),
                                                    };
                                                save_current_ui_settings(state);
                                                state.pending_sfx.push("item_put".to_string());
                                            }
                                        }
                                    }
                                }
                                // === World context menu targets ===
                                ContextMenuTarget::Player { id } => {
                                    // Options: 0=Attack, 1=Follow, 2=Trade, [3=Browse Shop if stall], N=Add Friend, N+1=Examine
                                    let player_has_stall =
                                        state.players.get(id).map_or(false, |p| p.has_stall);
                                    let mut ci = 0usize;
                                    let attack_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let follow_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let trade_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let browse_shop_idx = if player_has_stall {
                                        let idx = ci;
                                        ci += 1;
                                        Some(idx)
                                    } else {
                                        None
                                    };
                                    let add_friend_idx = {
                                        let idx = ci;
                                        ci += 1;
                                        idx
                                    };
                                    let examine_idx = ci;

                                    if *option_idx == attack_idx {
                                        commands.push(InputCommand::Target {
                                            entity_id: id.clone(),
                                        });
                                        state.auto_action_state =
                                            Some(crate::game::AutoActionState {
                                                target_type: "player".to_string(),
                                                target_id: id.clone(),
                                                action: "attack".to_string(),
                                                confirmed: false,
                                            });
                                        pathfind_and_attack_player(state, commands, id);
                                    } else if *option_idx == follow_idx {
                                        state.follow_target = Some(id.clone());
                                        if state.auto_action_state.is_some() {
                                            state.auto_action_state = None;
                                            commands.push(InputCommand::CancelAutoAction);
                                        }
                                        if let Some(local_id) = &state.local_player_id.clone() {
                                            if let Some(player) = state.players.get(local_id) {
                                                if let Some(target) = state.players.get(id) {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let tx = target.server_x.round() as i32;
                                                    let ty = target.server_y.round() as i32;
                                                    let mut occupied =
                                                        build_occupied_set(state, true, true);
                                                    occupied.remove(&(tx, ty));
                                                    const MAX_PATH_DISTANCE: i32 = 32;
                                                    if let Some((dest, path)) =
                                                        pathfinding::find_path_to_adjacent(
                                                            (px, py),
                                                            (tx, ty),
                                                            &state.chunk_manager,
                                                            &occupied,
                                                            MAX_PATH_DISTANCE,
                                                        )
                                                    {
                                                        state.auto_path = Some(PathState {
                                                            path,
                                                            current_index: 0,
                                                            destination: dest,
                                                            pickup_target: None,
                                                            interact_target: None,
                                                            interact_object_target: None,
                                                            waystone_target: None,
                                                            browse_stall_target: None,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    } else if *option_idx == trade_idx {
                                        // Send trade request
                                        commands.push(InputCommand::TradeRequest {
                                            target_id: id.clone(),
                                        });
                                    } else if browse_shop_idx == Some(*option_idx) {
                                        // Browse this player's stall
                                        commands.push(InputCommand::StallBrowse {
                                            player_id: id.clone(),
                                        });
                                    } else if *option_idx == add_friend_idx {
                                        if let Some(player) = state.players.get(id) {
                                            commands.push(InputCommand::SendFriendRequest {
                                                target_name: player.name.clone(),
                                            });
                                        }
                                    } else if *option_idx == examine_idx {
                                        if let Some(player) = state.players.get(id) {
                                            let msg = format!(
                                                "{} (level {})",
                                                player.name,
                                                player.combat_level()
                                            );
                                            state.push_system_chat(msg);
                                        }
                                    }
                                }
                                ContextMenuTarget::Npc { id } => {
                                    if let Some(npc) = state.npcs.get(id) {
                                        let is_attackable = npc.is_attackable();
                                        let is_altar = npc.is_altar;
                                        let is_merchant = npc.is_merchant;
                                        let is_banker = npc.is_banker;
                                        let is_slayer_master = npc.is_slayer_master;
                                        let has_station = npc.station_type.is_some();
                                        let npc_name = npc.display_name.clone();
                                        let npc_level = npc.level;
                                        let npc_entity_type = npc.entity_type.clone();
                                        let npc_id = id.clone();

                                        if is_attackable {
                                            // Options: 0=Attack, 1=Target, 2=Examine
                                            match option_idx {
                                                0 => {
                                                    commands.push(InputCommand::Target {
                                                        entity_id: npc_id.clone(),
                                                    });
                                                    state.auto_action_state =
                                                        Some(crate::game::AutoActionState {
                                                            target_type: "npc".to_string(),
                                                            target_id: npc_id.clone(),
                                                            action: "attack".to_string(),
                                                            confirmed: false,
                                                        });
                                                    pathfind_and_attack_npc(
                                                        state, commands, &npc_id,
                                                    );
                                                }
                                                1 => {
                                                    // Target only — select without attacking or moving
                                                    commands.push(InputCommand::Target {
                                                        entity_id: npc_id.clone(),
                                                    });
                                                }
                                                2 => {
                                                    let msg = format!(
                                                        "{} (level {})",
                                                        npc_name, npc_level
                                                    );
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_altar {
                                            // Options: 0=Pray, 1=Offer Bones, 2=Examine
                                            match option_idx {
                                                0 => {
                                                    // Pray at altar
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(
                                                                InputCommand::PrayAtAltar {
                                                                    altar_id: npc_id.to_string(),
                                                                },
                                                            );
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    // Offer Bones - open altar panel
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |state, _commands, npc_id| {
                                                            if let Some(npc) =
                                                                state.npcs.get(npc_id)
                                                            {
                                                                state.ui_state.altar_panel = Some(
                                                                    crate::game::AltarPanelState {
                                                                        altar_npc_id: npc_id
                                                                            .to_string(),
                                                                        altar_name: npc
                                                                            .display_name
                                                                            .clone(),
                                                                    },
                                                                );
                                                            }
                                                        },
                                                    );
                                                }
                                                2 => {
                                                    let msg =
                                                        format!("An altar dedicated to the gods.");
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if has_station {
                                            // Options: 0=Use, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |state, _commands, npc_id| {
                                                            if let Some(npc) =
                                                                state.npcs.get(npc_id)
                                                            {
                                                                match npc.station_type.as_deref() {
                                                                    Some("furnace") => {
                                                                        state
                                                                            .ui_state
                                                                            .furnace_station_type =
                                                                            "furnace".to_string();
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tile = Some((
                                                                            npc.x.round() as i32,
                                                                            npc.y.round() as i32,
                                                                        ));
                                                                        state.ui_state.furnace_selected_recipe = 0;
                                                                        state.ui_state.furnace_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tab = 0;
                                                                    }
                                                                    Some("fire_pit") => {
                                                                        state
                                                                            .ui_state
                                                                            .furnace_station_type =
                                                                            "fire_pit".to_string();
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tile = Some((
                                                                            npc.x.round() as i32,
                                                                            npc.y.round() as i32,
                                                                        ));
                                                                        state.ui_state.furnace_selected_recipe = 0;
                                                                        state.ui_state.furnace_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .furnace_tab = 0;
                                                                    }
                                                                    Some("anvil") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state.ui_state.anvil_open =
                                                                            true;
                                                                        state.ui_state.anvil_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.anvil_selected_recipe = 0;
                                                                        state
                                                                            .ui_state
                                                                            .anvil_scroll_offset =
                                                                            0.0;
                                                                        state
                                                                            .ui_state
                                                                            .anvil_quantity = 1;
                                                                        state.ui_state.anvil_tab =
                                                                            0;
                                                                    }
                                                                    Some("alchemy_station") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_open =
                                                                            true;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.alchemy_station_selected_recipe = 0;
                                                                        state.ui_state.alchemy_station_scroll_offset = 0.0;
                                                                        state.ui_state.alchemy_station_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_tab =
                                                                            0;
                                                                    }
                                                                    Some("workbench") => {
                                                                        state
                                                                            .ui_state
                                                                            .fletching_open = false;
                                                                        state
                                                                            .ui_state
                                                                            .alchemy_station_open =
                                                                            false;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_open = true;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_tile =
                                                                            Some((
                                                                                npc.x.round()
                                                                                    as i32,
                                                                                npc.y.round()
                                                                                    as i32,
                                                                            ));
                                                                        state.ui_state.workbench_selected_recipe = 0;
                                                                        state.ui_state.workbench_scroll_offset = 0.0;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_quantity = 1;
                                                                        state
                                                                            .ui_state
                                                                            .workbench_tab = 0;
                                                                    }
                                                                    _ => {
                                                                        _commands.push(InputCommand::Interact { npc_id: npc_id.to_string() });
                                                                    }
                                                                }
                                                            }
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_merchant {
                                            // Options: 0=Talk-to, 1=Trade, 2=Examine
                                            match option_idx {
                                                0 | 1 => {
                                                    // Both Talk-to and Trade interact with merchant
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                2 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_banker {
                                            // Options: 0=Bank, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    let msg = npc_name;
                                                    state.push_system_chat(msg);
                                                }
                                                _ => {}
                                            }
                                        } else if is_slayer_master {
                                            // Options: 0=Get Task, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    // Check requirements client-side for instant feedback
                                                    let (combat_req, slayer_req) =
                                                        slayer_master_requirements(
                                                            &npc_entity_type,
                                                        );
                                                    let player_combat = state
                                                        .get_local_player()
                                                        .map(|p| p.combat_level())
                                                        .unwrap_or(0);
                                                    let player_slayer = state
                                                        .get_local_player()
                                                        .map(|p| p.skills.slayer.level)
                                                        .unwrap_or(1);

                                                    if player_combat < combat_req {
                                                        state.push_system_chat(format!(
                                                            "You need combat level {} to get tasks from {}. (You are level {})",
                                                            combat_req, npc_name, player_combat
                                                        ));
                                                    } else if player_slayer < slayer_req {
                                                        state.push_system_chat(format!(
                                                            "You need slayer level {} to get tasks from {}. (You are level {})",
                                                            slayer_req, npc_name, player_slayer
                                                        ));
                                                    } else {
                                                        pathfind_and_interact_npc(
                                                            state,
                                                            commands,
                                                            &npc_id,
                                                            |_state, commands, npc_id| {
                                                                commands.push(
                                                                    InputCommand::SlayerGetTask {
                                                                        master_id: npc_id
                                                                            .to_string(),
                                                                    },
                                                                );
                                                            },
                                                        );
                                                    }
                                                }
                                                1 => {
                                                    let (combat_req, slayer_req) =
                                                        slayer_master_requirements(
                                                            &npc_entity_type,
                                                        );
                                                    if combat_req > 0 || slayer_req > 1 {
                                                        state.push_system_chat(format!(
                                                            "{} - Requires combat level {}, slayer level {}.",
                                                            npc_name, combat_req, slayer_req
                                                        ));
                                                    } else {
                                                        state.push_system_chat(format!(
                                                            "{} - Beginner slayer master.",
                                                            npc_name
                                                        ));
                                                    }
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            // Generic friendly NPC: 0=Talk-to, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    pathfind_and_interact_npc(
                                                        state,
                                                        commands,
                                                        &npc_id,
                                                        |_state, commands, npc_id| {
                                                            commands.push(InputCommand::Interact {
                                                                npc_id: npc_id.to_string(),
                                                            });
                                                        },
                                                    );
                                                }
                                                1 => {
                                                    state.push_system_chat(npc_name);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Tree {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Options: 0=Chop, 1=Examine
                                    match option_idx {
                                        0 => {
                                            let target_id =
                                                format!("{},{},{}", tile_x, tile_y, gid);
                                            state.auto_action_state =
                                                Some(crate::game::AutoActionState {
                                                    target_type: "resource".to_string(),
                                                    target_id: target_id.clone(),
                                                    action: "chop".to_string(),
                                                    confirmed: false,
                                                });
                                            pathfind_and_resource(
                                                state, commands, *tile_x, *tile_y, &target_id,
                                                "chop",
                                            );
                                        }
                                        1 => {
                                            let name = crate::game::tree_types::get_tree_info(*gid)
                                                .map(|info| info.name)
                                                .unwrap_or("Tree");
                                            state.push_system_chat(format!("{} tree.", name));
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::Rock {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Options: 0=Mine, 1=Examine
                                    match option_idx {
                                        0 => {
                                            let target_id =
                                                format!("{},{},{}", tile_x, tile_y, gid);
                                            state.auto_action_state =
                                                Some(crate::game::AutoActionState {
                                                    target_type: "resource".to_string(),
                                                    target_id: target_id.clone(),
                                                    action: "mine".to_string(),
                                                    confirmed: false,
                                                });
                                            pathfind_and_resource(
                                                state, commands, *tile_x, *tile_y, &target_id,
                                                "mine",
                                            );
                                        }
                                        1 => {
                                            let name = crate::game::ore_types::get_ore_info(*gid)
                                                .map(|info| info.name)
                                                .unwrap_or("Rock");
                                            state.push_system_chat(format!(
                                                "A rock containing {} ore.",
                                                name.to_lowercase()
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::MapObject {
                                    tile_x,
                                    tile_y,
                                    gid,
                                } => {
                                    // Obelisks: 0=Teleport, 1=Examine
                                    // Chests: 0=Open, 1=Examine
                                    // Other objects: 0=Interact, 1=Examine
                                    let tx = *tile_x;
                                    let ty = *tile_y;
                                    let is_chest = state.chest_positions.contains(&(tx, ty));
                                    match option_idx {
                                        0 => {
                                            if is_obelisk_gid(*gid) {
                                                // Walk to obelisk, then teleport directly
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let cdx = (px - tx).abs();
                                                    let cdy = (py - ty).abs();
                                                    if cdx <= 1 && cdy <= 1 {
                                                        commands.push(InputCommand::UseWaystone {
                                                            x: tx,
                                                            y: ty,
                                                        });
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (tx, ty),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: Some((tx, ty)),
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            } else if is_chest {
                                                // Walk to chest and open it
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let cdx = (px - tx).abs();
                                                    let cdy = (py - ty).abs();
                                                    if cdx <= 1 && cdy <= 1 {
                                                        commands.push(
                                                            InputCommand::InteractObject {
                                                                x: tx,
                                                                y: ty,
                                                            },
                                                        );
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (tx, ty),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: Some((
                                                                    tx, ty,
                                                                )),
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            } else {
                                                commands.push(InputCommand::InteractObject {
                                                    x: tx,
                                                    y: ty,
                                                });
                                            }
                                        }
                                        1 => {
                                            if is_chest {
                                                state.push_system_chat(
                                                    "A wooden storage chest.".to_string(),
                                                );
                                            } else if is_obelisk_gid(*gid) {
                                                state.push_system_chat("An ancient obelisk humming with magical energy.".to_string());
                                            } else {
                                                match get_map_object_name(*gid) {
                                                    Some(name) => state.push_system_chat(format!(
                                                        "A {}.",
                                                        name.to_lowercase()
                                                    )),
                                                    None => state.push_system_chat(
                                                        "Nothing interesting.".to_string(),
                                                    ),
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::GatheringSpot { marker_index } => {
                                    // Options: 0=Fish/Gather, 1=Examine
                                    match option_idx {
                                        0 => {
                                            if let Some(marker) =
                                                state.gathering_markers.get(*marker_index)
                                            {
                                                let marker_x = marker.x;
                                                let marker_y = marker.y;
                                                // Pathfind to marker and start gathering
                                                if let Some(player) = state.get_local_player() {
                                                    let px = player.server_x.round() as i32;
                                                    let py = player.server_y.round() as i32;
                                                    let dx = (px - marker_x).abs();
                                                    let dy = (py - marker_y).abs();
                                                    if dx <= 1 && dy <= 1 {
                                                        commands.push(
                                                            InputCommand::StartGathering {
                                                                marker_x,
                                                                marker_y,
                                                            },
                                                        );
                                                    } else {
                                                        let occupied =
                                                            build_occupied_set(state, true, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some((dest, path)) =
                                                            pathfinding::find_path_to_adjacent(
                                                                (px, py),
                                                                (marker_x, marker_y),
                                                                &state.chunk_manager,
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: dest,
                                                                pickup_target: None,
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                            // Player will need to interact again when they arrive
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        1 => {
                                            if let Some(marker) =
                                                state.gathering_markers.get(*marker_index)
                                            {
                                                state.push_system_chat(format!(
                                                    "A {} spot.",
                                                    marker.skill
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::GroundItem { id } => {
                                    // Options: 0=Pick up, 1=Examine
                                    match option_idx {
                                        0 => {
                                            if let Some(item) = state.ground_items.get(id) {
                                                let item_x = item.x.round() as i32;
                                                let item_y = item.y.round() as i32;
                                                let item_id = item.id.clone();
                                                const PICKUP_RANGE: f32 = 2.0;
                                                if let Some(player) = state.get_local_player() {
                                                    let dx = item.x - player.x;
                                                    let dy = item.y - player.y;
                                                    let dist = (dx * dx + dy * dy).sqrt();
                                                    if dist < PICKUP_RANGE {
                                                        commands
                                                            .push(InputCommand::Pickup { item_id });
                                                    } else {
                                                        // Pathfind to item
                                                        let px = player.server_x.round() as i32;
                                                        let py = player.server_y.round() as i32;
                                                        let occupied =
                                                            build_occupied_set(state, true, true);
                                                        const MAX_PATH_DISTANCE: i32 = 32;
                                                        if let Some(path) =
                                                            find_path_with_optimistic_splice(
                                                                state,
                                                                (px, py),
                                                                (item_x, item_y),
                                                                &occupied,
                                                                MAX_PATH_DISTANCE,
                                                            )
                                                        {
                                                            state.auto_path = Some(PathState {
                                                                path,
                                                                current_index: 0,
                                                                destination: (item_x, item_y),
                                                                pickup_target: Some(item_id),
                                                                interact_target: None,
                                                                interact_object_target: None,
                                                                waystone_target: None,
                                                                browse_stall_target: None,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        1 => {
                                            if let Some(item) = state.ground_items.get(id) {
                                                let item_def = state
                                                    .item_registry
                                                    .get_or_placeholder(&item.item_id);
                                                state.push_system_chat(format!(
                                                    "{}: {}",
                                                    item_def.display_name, item_def.description
                                                ));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ContextMenuTarget::FarmingPatch { patch_id } => {
                                    if let Some(patch) = state.farming_patches.get(patch_id) {
                                        let patch_state = patch.state.clone();
                                        let patch_x = patch.x;
                                        let patch_y = patch.y;
                                        let pid = patch_id.clone();
                                        if patch_state == "harvestable" {
                                            // Options: 0=Harvest, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    if let Some(player) = state.get_local_player() {
                                                        let px = player.server_x.round() as i32;
                                                        let py = player.server_y.round() as i32;
                                                        let cdx = (px - patch_x).abs();
                                                        let cdy = (py - patch_y).abs();
                                                        if cdx <= 1 && cdy <= 1 {
                                                            commands.push(
                                                                InputCommand::HarvestCrop {
                                                                    patch_id: pid,
                                                                },
                                                            );
                                                        } else {
                                                            let occupied = build_occupied_set(
                                                                state, true, true,
                                                            );
                                                            const MAX_PATH_DISTANCE: i32 = 32;
                                                            if let Some((dest, path)) =
                                                                pathfinding::find_path_to_adjacent(
                                                                    (px, py),
                                                                    (patch_x, patch_y),
                                                                    &state.chunk_manager,
                                                                    &occupied,
                                                                    MAX_PATH_DISTANCE,
                                                                )
                                                            {
                                                                state.auto_path = Some(PathState {
                                                                    path,
                                                                    current_index: 0,
                                                                    destination: dest,
                                                                    pickup_target: None,
                                                                    interact_target: None,
                                                                    interact_object_target: None,
                                                                    waystone_target: None,
                                                                    browse_stall_target: None,
                                                                });
                                                                state.pending_harvest_patch =
                                                                    Some(pid);
                                                            }
                                                        }
                                                    }
                                                }
                                                1 => {
                                                    state.push_system_chat(
                                                        "This crop is ready to harvest."
                                                            .to_string(),
                                                    );
                                                }
                                                _ => {}
                                            }
                                        } else if patch_state == "empty" {
                                            // Options: 0=Plant, 1=Examine
                                            match option_idx {
                                                0 => {
                                                    // TODO: Open seed selection UI or plant first seed
                                                    state.push_system_chat(
                                                        "Use a seed on this patch to plant it."
                                                            .to_string(),
                                                    );
                                                }
                                                1 => {
                                                    state.push_system_chat(
                                                        "An empty farming patch.".to_string(),
                                                    );
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            // Growing state - only Examine
                                            if *option_idx == 0 {
                                                state.push_system_chat(
                                                    "A farming patch with something growing."
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Tile { x, y } => {
                                    // Options: 0=Walk here
                                    if *option_idx == 0 {
                                        pathfind_to_tile(state, commands, *x, *y);
                                    }
                                }
                                ContextMenuTarget::HotkeySlot(slot_idx) => {
                                    // Options: 0=Clear Slot
                                    if *option_idx == 0 {
                                        state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                            crate::game::hotkey::HotkeySlotBinding::Empty;
                                        save_current_ui_settings(state);
                                    }
                                }
                                ContextMenuTarget::Spell(spell_id) => {
                                    // Options: 0=Hotkey 1, 1=Hotkey 2, 2=Hotkey 3
                                    if *option_idx < 3 {
                                        state.ui_state.hotkey_bar.active_mut().slots[*option_idx] =
                                            crate::game::hotkey::HotkeySlotBinding::Spell {
                                                spell_id: spell_id.clone(),
                                            };
                                        save_current_ui_settings(state);
                                        state.pending_sfx.push("item_put".to_string());
                                    }
                                }
                                ContextMenuTarget::QuestTracker => {
                                    // Options: 0=Minimize or Expand
                                    if *option_idx == 0 {
                                        state.ui_state.quest_tracker_minimized =
                                            !state.ui_state.quest_tracker_minimized;
                                        save_current_ui_settings(state);
                                    }
                                }
                                ContextMenuTarget::ChatTab => {
                                    // Options: 0=Toggle system messages
                                    if *option_idx == 0 {
                                        state.ui_state.hide_system_in_public =
                                            !state.ui_state.hide_system_in_public;
                                        save_current_ui_settings(state);
                                    }
                                }
                                ContextMenuTarget::BankSlot(idx) => {
                                    if let Some(Some((item_id, qty))) =
                                        state.ui_state.bank_slots.get(*idx)
                                    {
                                        let item_id = item_id.clone();
                                        let qty = *qty;
                                        let has_many = qty > 1;
                                        match option_idx {
                                            0 => {
                                                // Withdraw 1
                                                commands.push(InputCommand::BankWithdraw {
                                                    item_id,
                                                    quantity: 1,
                                                });
                                                state.pending_sfx.push("enter".to_string());
                                            }
                                            1 if has_many => {
                                                // Withdraw X
                                                state.ui_state.bank_quantity_dialog =
                                                    Some(BankQuantityDialog {
                                                        input: String::new(),
                                                        cursor: 0,
                                                        action: BankQuantityAction::WithdrawItem,
                                                        item_id: Some(item_id),
                                                        max_quantity: qty,
                                                    });
                                            }
                                            2 if has_many => {
                                                // Withdraw All
                                                commands.push(InputCommand::BankWithdraw {
                                                    item_id,
                                                    quantity: qty,
                                                });
                                                state.pending_sfx.push("enter".to_string());
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                ContextMenuTarget::BankInventorySlot(idx) => {
                                    if let Some(Some(slot)) = state.inventory.slots.get(*idx) {
                                        let item_id = slot.item_id.clone();
                                        let qty = slot.quantity;
                                        let has_many = qty > 1;
                                        match option_idx {
                                            0 => {
                                                // Deposit 1
                                                commands.push(InputCommand::BankDeposit {
                                                    item_id,
                                                    quantity: 1,
                                                });
                                                state.pending_sfx.push("enter".to_string());
                                            }
                                            1 if has_many => {
                                                // Deposit X
                                                state.ui_state.bank_quantity_dialog =
                                                    Some(BankQuantityDialog {
                                                        input: String::new(),
                                                        cursor: 0,
                                                        action: BankQuantityAction::DepositItem,
                                                        item_id: Some(item_id),
                                                        max_quantity: qty,
                                                    });
                                            }
                                            2 if has_many => {
                                                // Deposit All
                                                commands.push(InputCommand::BankDeposit {
                                                    item_id,
                                                    quantity: qty,
                                                });
                                                state.pending_sfx.push("enter".to_string());
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            return true;
                        }
                        _ => {
                            // Clicked somewhere else, close menu
                            state.ui_state.context_menu = None;
                        }
                    }
                }
            } else if mouse_clicked || mouse_right_clicked {
                // Clicked outside any element, close menu
                state.ui_state.context_menu = None;
            }

            // Escape closes context menu
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.context_menu = None;
                return true;
            }
        }

        false
    }
}
