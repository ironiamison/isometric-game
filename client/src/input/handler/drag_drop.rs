use super::*;

impl InputHandler {
    pub(super) fn handle_drag_drop(
        &self,
        state: &mut GameState,
        clicked_element: Option<&UiElementId>,
        audio: &mut AudioManager,
        commands: &mut Vec<InputCommand>,
    ) -> bool {
        let Some(drag) = state.ui_state.drag_state.take() else {
            return false;
        };

        if let Some(element) = clicked_element {
            match element {
                UiElementId::InventorySlot(to_idx) => match &drag.source {
                    DragSource::Inventory(from_idx) => {
                        if *from_idx != *to_idx {
                            state.inventory.swap_slots(*from_idx, *to_idx);
                            audio.play_sfx("item_put");

                            commands.push(InputCommand::SwapSlots {
                                from_slot: *from_idx as u8,
                                to_slot: *to_idx as u8,
                            });
                        }
                    }
                    DragSource::Equipment(slot_type) => {
                        if state
                            .inventory
                            .slots
                            .get(*to_idx)
                            .map(|s| s.is_none())
                            .unwrap_or(false)
                        {
                            state
                                .inventory
                                .set_slot(*to_idx, drag.item_id.clone(), drag.quantity);

                            if let Some(local_id) = &state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(local_id) {
                                    match slot_type.as_str() {
                                        "head" => player.equipped_head = None,
                                        "body" => player.equipped_body = None,
                                        "weapon" => player.equipped_weapon = None,
                                        "back" => player.equipped_back = None,
                                        "feet" => player.equipped_feet = None,
                                        "ring" => player.equipped_ring = None,
                                        "gloves" => player.equipped_gloves = None,
                                        "necklace" => player.equipped_necklace = None,
                                        "belt" => player.equipped_belt = None,
                                        _ => {}
                                    }
                                }
                            }
                        }

                        audio.play_sfx("item_put");
                        commands.push(InputCommand::Unequip {
                            slot_type: slot_type.clone(),
                            target_slot: Some(*to_idx as u8),
                        });
                    }
                    DragSource::Spell(_) => {}
                },
                UiElementId::QuickSlot(slot_idx) | UiElementId::HotkeySettingsSlot(slot_idx) => {
                    match &drag.source {
                        DragSource::Inventory(inv_idx) => {
                            if let Some(Some(slot)) = state.inventory.slots.get(*inv_idx) {
                                state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                    crate::game::hotkey::HotkeySlotBinding::Item {
                                        item_id: slot.item_id.clone(),
                                    };
                                save_current_ui_settings(state);
                                audio.play_sfx("item_put");
                            }
                        }
                        DragSource::Spell(spell_id) => {
                            state.ui_state.hotkey_bar.active_mut().slots[*slot_idx] =
                                crate::game::hotkey::HotkeySlotBinding::Spell {
                                    spell_id: spell_id.clone(),
                                };
                            save_current_ui_settings(state);
                            audio.play_sfx("item_put");
                        }
                        DragSource::Equipment(_) => {}
                    }
                }
                UiElementId::EquipmentSlot(target_slot_type) => match &drag.source {
                    DragSource::Inventory(from_idx) => {
                        let item_def = state.item_registry.get_or_placeholder(&drag.item_id);
                        let can_equip = if let Some(ref equip) = item_def.equipment {
                            let slot_matches = equip.slot_type == *target_slot_type;
                            let level_ok = state
                                .get_local_player()
                                .map(|p| {
                                    p.skills.attack.level >= equip.attack_level_required
                                        && p.skills.defence.level >= equip.defence_level_required
                                        && p.skills.ranged.level >= equip.ranged_level_required
                                })
                                .unwrap_or(false);
                            slot_matches && level_ok
                        } else {
                            false
                        };

                        if can_equip {
                            if let Some(local_id) = &state.local_player_id.clone() {
                                if let Some(player) = state.players.get_mut(local_id) {
                                    match target_slot_type.as_str() {
                                        "head" => player.equipped_head = Some(drag.item_id.clone()),
                                        "body" => player.equipped_body = Some(drag.item_id.clone()),
                                        "weapon" => {
                                            player.equipped_weapon = Some(drag.item_id.clone())
                                        }
                                        "back" => player.equipped_back = Some(drag.item_id.clone()),
                                        "feet" => player.equipped_feet = Some(drag.item_id.clone()),
                                        "ring" => player.equipped_ring = Some(drag.item_id.clone()),
                                        "gloves" => {
                                            player.equipped_gloves = Some(drag.item_id.clone())
                                        }
                                        "necklace" => {
                                            player.equipped_necklace = Some(drag.item_id.clone())
                                        }
                                        "belt" => player.equipped_belt = Some(drag.item_id.clone()),
                                        _ => {}
                                    }
                                }
                            }
                            state.inventory.clear_slot(*from_idx);
                            audio.play_sfx("item_put");

                            commands.push(InputCommand::Equip {
                                slot_index: *from_idx as u8,
                            });
                        }
                    }
                    DragSource::Equipment(source_slot_type) => {
                        if source_slot_type != target_slot_type {}
                    }
                    DragSource::Spell(_) => {}
                },
                UiElementId::ChestSlot(_) | UiElementId::ChestScrollArea => {
                    if state.ui_state.chest_open {
                        if let DragSource::Inventory(from_idx) = &drag.source {
                            commands.push(InputCommand::ChestDeposit {
                                chest_id: state.ui_state.chest_id.clone(),
                                inventory_slot: *from_idx as u8,
                            });
                            audio.play_sfx("item_put");
                        }
                    }
                }
                _ => {}
            }
        } else if let DragSource::Inventory(from_idx) = &drag.source {
            if let Some((tile_x, tile_y)) = state.hovered_tile {
                if let Some(player) = state.get_local_player() {
                    let player_x = player.x.round() as i32;
                    let player_y = player.y.round() as i32;
                    let dx = (tile_x - player_x).abs();
                    let dy = (tile_y - player_y).abs();
                    // For farming patches, allow the drop from anywhere adjacent to the whole
                    // footprint, so dropping on a far bed tile works while standing at its edge.
                    let footprint_adjacent = state
                        .farming_patch_positions
                        .get(&(tile_x, tile_y))
                        .and_then(|id| state.farming_patches.get(id))
                        .map(|p| {
                            let cx = player_x.clamp(p.x, p.x + p.width.max(1) as i32 - 1);
                            let cy = player_y.clamp(p.y, p.y + p.height.max(1) as i32 - 1);
                            (player_x - cx).abs() <= 1 && (player_y - cy).abs() <= 1
                        })
                        .unwrap_or(false);
                    let is_adjacent = (dx <= 1 && dy <= 1) || footprint_adjacent;

                    if is_adjacent {
                        let is_seed_on_patch = if let Some(patch_id) =
                            state.farming_patch_positions.get(&(tile_x, tile_y))
                        {
                            if let Some(patch) = state.farming_patches.get(patch_id) {
                                if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                    let item_id = slot.item_id.clone();
                                    if patch.state == "empty" && item_id.ends_with("_seed") {
                                        commands.push(InputCommand::PlantSeed {
                                            patch_id: patch_id.clone(),
                                            item_id,
                                        });
                                        audio.play_sfx("item_put");
                                        true
                                    } else if item_id == "compost"
                                        && !patch.composted
                                        && matches!(
                                            patch.state.as_str(),
                                            "empty" | "growing" | "harvestable"
                                        )
                                    {
                                        commands.push(InputCommand::ApplyCompost {
                                            patch_id: patch_id.clone(),
                                            item_id,
                                        });
                                        audio.play_sfx("item_put");
                                        true
                                    } else if item_id == "plant_cure_potion"
                                        && patch.state == "diseased"
                                    {
                                        commands.push(InputCommand::CurePatch {
                                            patch_id: patch_id.clone(),
                                        });
                                        audio.play_sfx("item_put");
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        let is_bones_on_altar = if !is_seed_on_patch {
                            if let Some(Some(slot)) = state.inventory.slots.get(*from_idx) {
                                if slot.item_id.contains("bones") {
                                    let mut altar_id = None;
                                    for (npc_id, npc) in &state.npcs {
                                        if npc.is_altar
                                            && npc.x.round() as i32 == tile_x
                                            && npc.y.round() as i32 == tile_y
                                        {
                                            altar_id = Some(npc_id.clone());
                                            break;
                                        }
                                    }
                                    if let Some(aid) = altar_id {
                                        commands.push(InputCommand::OfferBones {
                                            slot: *from_idx as u8,
                                            altar_id: aid,
                                        });
                                        audio.play_sfx("item_put");
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !is_seed_on_patch && !is_bones_on_altar {
                            let ctrl_held = is_key_down(KeyCode::LeftControl)
                                || is_key_down(KeyCode::RightControl)
                                || is_key_down(KeyCode::LeftSuper)
                                || is_key_down(KeyCode::RightSuper);

                            let quantity = if ctrl_held { 1 } else { drag.quantity as u32 };

                            commands.push(InputCommand::DropItem {
                                slot_index: *from_idx as u8,
                                quantity,
                                target_x: Some(tile_x),
                                target_y: Some(tile_y),
                            });
                            audio.play_sfx("item_put");
                        }
                    }
                }
            }
        }

        true
    }
}
