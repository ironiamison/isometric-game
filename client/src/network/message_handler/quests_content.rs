use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "showDialogue" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                let mut speaker = extract_string(value, "speaker").unwrap_or_default();
                let text = extract_string(value, "text").unwrap_or_default();

                // If the NPC is an adventurer guide, always use its canonical
                // speaker name so the custom guide UI renders instead of the
                // generic dialogue box.
                if let Some(npc) = state.npcs.get(&npc_id) {
                    if npc.entity_type == "adventurer_guide" {
                        speaker = "Adventurer Guide".to_string();
                    }
                }

                // Parse choices array
                let mut choices = Vec::new();
                if let Some(choices_arr) = extract_array(value, "choices") {
                    for choice_value in choices_arr {
                        let id = extract_string(choice_value, "id").unwrap_or_default();
                        let choice_text = extract_string(choice_value, "text").unwrap_or_default();
                        choices.push(DialogueChoice {
                            id,
                            text: choice_text,
                        });
                    }
                }

                // If Old Thomas's tutorial dialogue but tutorial is already done,
                // show a friendly post-tutorial greeting instead
                let (quest_id, text, choices) = if quest_id == "__tutorial__"
                    && (crate::settings::load_tutorial_completed()
                        || state.tutorial.as_ref().map_or(false, |t| t.is_done()))
                {
                    (
                        String::new(),
                        "Good to see you again, friend! You're doing great out there. Remember, the Adventurer Guide can help you find your next challenge!".to_string(),
                        vec![DialogueChoice {
                            id: "close".to_string(),
                            text: "Thanks, Old Thomas!".to_string(),
                        }],
                    )
                } else {
                    (quest_id, text, choices)
                };

                log::info!(
                    "Showing dialogue from {}: {} ({} choices)",
                    speaker,
                    text,
                    choices.len()
                );

                let already_open = state
                    .ui_state
                    .active_dialogue
                    .as_ref()
                    .map(|d| d.npc_id == npc_id)
                    .unwrap_or(false);

                state.ui_state.dialogue_scroll_offset = 0.0;
                state.ui_state.dialogue_touch_scroll_id = None;
                state.ui_state.dialogue_touch_dragged = false;
                state.ui_state.selected_inventory_slot = None;
                if !quest_id.starts_with("adventure_board:") {
                    clear_adventure_board_dialogue(state);
                }
                state.ui_state.active_dialogue = Some(ActiveDialogue {
                    quest_id,
                    npc_id,
                    speaker,
                    text,
                    choices,
                    show_time: macroquad::time::get_time(),
                });

                // Keep the custom Adventurer Guide panel focused on the quest currently discussed.
                if let Some(dialogue) = &state.ui_state.active_dialogue {
                    if dialogue.speaker.eq_ignore_ascii_case("Adventurer Guide") {
                        let selected = match dialogue.quest_id.as_str() {
                            "adventurer_tier_1" => Some((0, 0)),
                            "adventurer_tier_2" => Some((0, 1)),
                            "adventurer_tier_3" => Some((0, 2)),
                            "skilling_tier_1" | "woodcutting_tier_1" | "fishing_tier_1"
                            | "alchemy_tier_1" => Some((1, 0)),
                            "skilling_tier_2" | "woodcutting_tier_2" | "fishing_tier_2"
                            | "alchemy_tier_2" => Some((1, 1)),
                            "skilling_tier_3" | "woodcutting_tier_3" | "fishing_tier_3"
                            | "alchemy_tier_3" => Some((1, 2)),
                            _ => None,
                        };
                        if let Some((tab_idx, tier_idx)) = selected {
                            state.ui_state.adventurer_selected_tab = tab_idx;
                            state.ui_state.adventurer_selected_tier = tier_idx;
                        }
                    }
                }

                if !already_open {
                    state.pending_sfx.push("ui_open".to_string());
                }
            }
        }
        "adventureBoardState" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                let previous_kind = state.ui_state.adventure_board.as_ref().and_then(|board| {
                    board
                        .offers
                        .get(state.ui_state.adventure_board_selected_offer)
                        .map(|offer| offer.kind_id.clone())
                });
                let already_open = state
                    .ui_state
                    .active_dialogue
                    .as_ref()
                    .map(|d| d.quest_id == format!("adventure_board:{}", npc_id))
                    .unwrap_or(false);

                let mut offers = Vec::new();
                if let Some(offers_arr) = extract_array(value, "offers") {
                    for offer_value in offers_arr {
                        let mut difficulties = Vec::new();
                        if let Some(diff_arr) = extract_array(offer_value, "difficulties") {
                            for diff_value in diff_arr {
                                difficulties.push(AdventureBoardDifficultyInfo {
                                    difficulty_id: extract_string(diff_value, "difficulty_id")
                                        .unwrap_or_default(),
                                    difficulty_name: extract_string(diff_value, "difficulty_name")
                                        .unwrap_or_default(),
                                    level_required: extract_i32(diff_value, "level_required")
                                        .unwrap_or(0),
                                    unlocked: extract_bool(diff_value, "unlocked").unwrap_or(false),
                                    reward_xp: extract_i64(diff_value, "reward_xp").unwrap_or(0),
                                    reward_gold: extract_i32(diff_value, "reward_gold")
                                        .unwrap_or(0),
                                });
                            }
                        }

                        offers.push(AdventureBoardOfferInfo {
                            kind_id: extract_string(offer_value, "kind_id").unwrap_or_default(),
                            kind_name: extract_string(offer_value, "kind_name").unwrap_or_default(),
                            description: extract_string(offer_value, "description")
                                .unwrap_or_default(),
                            skill_level: extract_i32(offer_value, "skill_level").unwrap_or(0),
                            difficulties,
                        });
                    }
                }

                let active_contract = extract_map_field(value, "active_contract")
                    .filter(|contract| !contract.is_nil())
                    .map(|contract| AdventureBoardActiveContractInfo {
                        kind_id: extract_string(contract, "kind_id").unwrap_or_default(),
                        kind_name: extract_string(contract, "kind_name").unwrap_or_default(),
                        difficulty_name: extract_string(contract, "difficulty_name")
                            .unwrap_or_default(),
                        task_text: extract_string(contract, "task_text").unwrap_or_default(),
                        progress_label: extract_string(contract, "progress_label")
                            .unwrap_or_default(),
                        amount_required: extract_i32(contract, "amount_required").unwrap_or(0),
                        amount_completed: extract_i32(contract, "amount_completed").unwrap_or(0),
                        giver_name: extract_string(contract, "giver_name").unwrap_or_default(),
                        reward_xp: extract_i64(contract, "reward_xp").unwrap_or(0),
                        reward_gold: extract_i32(contract, "reward_gold").unwrap_or(0),
                        bonus_item_text: extract_string(contract, "bonus_item_text")
                            .unwrap_or_default(),
                        can_claim: extract_bool(contract, "can_claim").unwrap_or(false),
                    });

                let stats = extract_map_field(value, "stats")
                    .map(|stats| AdventureBoardStatsInfo {
                        contracts_completed: extract_i32(stats, "contracts_completed").unwrap_or(0),
                        total_gold_earned: extract_i32(stats, "total_gold_earned").unwrap_or(0),
                        total_xp_earned: extract_i64(stats, "total_xp_earned").unwrap_or(0),
                    })
                    .unwrap_or(AdventureBoardStatsInfo {
                        contracts_completed: 0,
                        total_gold_earned: 0,
                        total_xp_earned: 0,
                    });

                // Parse crafting orders
                let mut crafting_orders = Vec::new();
                if let Some(orders_arr) = extract_array(value, "crafting_orders") {
                    for order_value in orders_arr {
                        let mut items = Vec::new();
                        if let Some(items_arr) = extract_array(order_value, "items") {
                            for item_value in items_arr {
                                items.push(CraftingOrderItemInfo {
                                    item_id: extract_string(item_value, "item_id")
                                        .unwrap_or_default(),
                                    item_name: extract_string(item_value, "item_name")
                                        .unwrap_or_default(),
                                    quantity: extract_i32(item_value, "quantity").unwrap_or(0),
                                });
                            }
                        }
                        let mut reward_xp = Vec::new();
                        if let Some(xp_arr) = extract_array(order_value, "reward_xp") {
                            for xp_value in xp_arr {
                                let skill = extract_string(xp_value, "skill").unwrap_or_default();
                                let amount = extract_i64(xp_value, "amount").unwrap_or(0);
                                reward_xp.push((skill, amount));
                            }
                        }
                        crafting_orders.push(CraftingOrderOfferInfo {
                            order_id: extract_string(order_value, "order_id").unwrap_or_default(),
                            tier: extract_string(order_value, "tier").unwrap_or_default(),
                            skill: extract_string(order_value, "skill").unwrap_or_default(),
                            min_level: extract_i32(order_value, "min_level").unwrap_or(0),
                            items,
                            reward_gold: extract_i32(order_value, "reward_gold").unwrap_or(0),
                            reward_xp,
                            reward_marks: extract_i32(order_value, "reward_marks").unwrap_or(0),
                        });
                    }
                }

                // Parse active crafting order
                let crafting_order_active = extract_map_field(value, "crafting_order_active")
                    .filter(|v| !v.is_nil())
                    .map(|order| {
                        let mut items = Vec::new();
                        if let Some(items_arr) = extract_array(order, "items") {
                            for item_value in items_arr {
                                items.push(CraftingOrderItemInfo {
                                    item_id: extract_string(item_value, "item_id")
                                        .unwrap_or_default(),
                                    item_name: extract_string(item_value, "item_name")
                                        .unwrap_or_default(),
                                    quantity: extract_i32(item_value, "quantity").unwrap_or(0),
                                });
                            }
                        }
                        CraftingOrderActiveInfo {
                            order_id: extract_string(order, "order_id").unwrap_or_default(),
                            tier: extract_string(order, "tier").unwrap_or_default(),
                            skill: extract_string(order, "skill").unwrap_or_default(),
                            items,
                            reward_gold: extract_i32(order, "reward_gold").unwrap_or(0),
                            reward_marks: extract_i32(order, "reward_marks").unwrap_or(0),
                            can_claim: extract_bool(order, "can_claim").unwrap_or(false),
                        }
                    });

                // Parse crafting order stats
                let crafting_order_stats = extract_map_field(value, "crafting_order_stats")
                    .map(|s| CraftingOrderStatsInfo {
                        orders_completed: extract_i32(s, "orders_completed").unwrap_or(0),
                        masterwork_completed: extract_i32(s, "masterwork_completed").unwrap_or(0),
                        commission_marks: extract_i32(s, "commission_marks").unwrap_or(0),
                    })
                    .unwrap_or(CraftingOrderStatsInfo {
                        orders_completed: 0,
                        masterwork_completed: 0,
                        commission_marks: 0,
                    });

                let daily_contracts_completed =
                    extract_i32(value, "daily_contracts_completed").unwrap_or(0);
                let daily_contract_limit = extract_i32(value, "daily_contract_limit").unwrap_or(5);

                state.ui_state.adventure_board = Some(AdventureBoardPanelState {
                    npc_id: npc_id.clone(),
                    offers,
                    active_contract,
                    stats,
                    crafting_orders,
                    crafting_order_active,
                    crafting_order_stats,
                    daily_contracts_completed,
                    daily_contract_limit,
                });

                if let Some(target_kind) = previous_kind {
                    if let Some(board) = state.ui_state.adventure_board.as_ref() {
                        if let Some(idx) = board
                            .offers
                            .iter()
                            .position(|offer| offer.kind_id == target_kind)
                        {
                            state.ui_state.adventure_board_selected_offer = idx;
                        }
                    }
                } else if let Some(board) = state.ui_state.adventure_board.as_ref() {
                    if let Some(active_kind) = board
                        .active_contract
                        .as_ref()
                        .map(|contract| contract.kind_id.as_str())
                    {
                        if let Some(idx) = board
                            .offers
                            .iter()
                            .position(|offer| offer.kind_id == active_kind)
                        {
                            state.ui_state.adventure_board_selected_offer = idx;
                        }
                    }
                }

                if let Some(board) = state.ui_state.adventure_board.as_ref() {
                    if board.offers.is_empty() {
                        state.ui_state.adventure_board_selected_offer = 0;
                    } else {
                        state.ui_state.adventure_board_selected_offer = state
                            .ui_state
                            .adventure_board_selected_offer
                            .min(board.offers.len().saturating_sub(1));
                    }
                }

                state.ui_state.active_dialogue = Some(ActiveDialogue {
                    quest_id: format!("adventure_board:{}", npc_id),
                    npc_id,
                    speaker: "Adventure Board".to_string(),
                    text: String::new(),
                    choices: Vec::new(),
                    show_time: macroquad::time::get_time(),
                });
                state.ui_state.dialogue_scroll_offset = 0.0;
                state.ui_state.dialogue_touch_scroll_id = None;
                state.ui_state.dialogue_touch_dragged = false;
                state.ui_state.selected_inventory_slot = None;

                if !already_open {
                    state.pending_sfx.push("ui_open".to_string());
                }
            }
        }
        "questAccepted" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let quest_name = extract_string(value, "quest_name").unwrap_or_default();
                let accepted_id = quest_id.clone();

                // Parse objectives
                let mut objectives = Vec::new();
                if let Some(obj_arr) = extract_array(value, "objectives") {
                    for obj_value in obj_arr {
                        let id = extract_string(obj_value, "id").unwrap_or_default();
                        let description =
                            extract_string(obj_value, "description").unwrap_or_default();
                        let current = extract_i32(obj_value, "current").unwrap_or(0);
                        let target = extract_i32(obj_value, "target").unwrap_or(1);
                        objectives.push(QuestObjective {
                            id,
                            description,
                            current,
                            target,
                            completed: current >= target,
                        });
                    }
                }

                log::info!("Quest accepted: {} - {}", quest_id, quest_name);

                // Add to active quests (or update if exists)
                if let Some(existing) = state
                    .ui_state
                    .active_quests
                    .iter_mut()
                    .find(|q| q.id == quest_id)
                {
                    existing.objectives = objectives;
                } else {
                    state.ui_state.active_quests.push(ActiveQuest {
                        id: quest_id,
                        name: quest_name,
                        objectives,
                    });
                }
                state.ui_state.completed_quest_ids.remove(&accepted_id);

                // Don't close dialogue here - let user read the quest acceptance message
                // Dialogue will close when user presses continue or server sends dialogueClosed
            }
        }
        "questStateSync" => {
            if let Some(value) = data {
                state.ui_state.completed_quest_ids.clear();
                if let Some(ids) = extract_array(value, "completed_quest_ids") {
                    for id_value in ids {
                        if let Some(id) = id_value.as_str() {
                            state.ui_state.completed_quest_ids.insert(id.to_string());
                        }
                    }
                }
            }
        }
        "questCatalog" => {
            if let Some(value) = data {
                state.ui_state.quest_catalog.clear();
                if let Some(quests) = extract_array(value, "quests") {
                    for q in quests {
                        let quest_id = extract_string(q, "quest_id").unwrap_or_default();
                        let name = extract_string(q, "name").unwrap_or_default();
                        let description = extract_string(q, "description").unwrap_or_default();
                        let giver_npc_name =
                            extract_string(q, "giver_npc_name").unwrap_or_default();
                        let level_required = extract_i32(q, "level_required").unwrap_or(0);
                        let required_quest_id = extract_string(q, "required_quest_id");
                        let required_quest_name = extract_string(q, "required_quest_name");
                        let mut objectives = Vec::new();
                        if let Some(obj_arr) = extract_array(q, "objectives") {
                            for obj in obj_arr {
                                let id = extract_string(obj, "id").unwrap_or_default();
                                let description =
                                    extract_string(obj, "description").unwrap_or_default();
                                let target = extract_i32(obj, "target").unwrap_or(1);
                                objectives.push(CatalogObjective {
                                    id,
                                    description,
                                    target,
                                });
                            }
                        }
                        state.ui_state.quest_catalog.push(QuestCatalogEntry {
                            quest_id,
                            name,
                            description,
                            giver_npc_name,
                            level_required,
                            required_quest_id,
                            required_quest_name,
                            objectives,
                        });
                    }
                }
                log::info!(
                    "Received quest catalog with {} quests",
                    state.ui_state.quest_catalog.len()
                );
            }
        }
        "questObjectiveProgress" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let objective_id = extract_string(value, "objective_id").unwrap_or_default();
                let current = extract_i32(value, "current").unwrap_or(0);
                let target = extract_i32(value, "target").unwrap_or(1);

                log::debug!(
                    "Quest objective progress: {}:{} = {}/{}",
                    quest_id,
                    objective_id,
                    current,
                    target
                );

                // Update the objective in the active quest
                if let Some(quest) = state
                    .ui_state
                    .active_quests
                    .iter_mut()
                    .find(|q| q.id == quest_id)
                {
                    if let Some(obj) = quest.objectives.iter_mut().find(|o| o.id == objective_id) {
                        obj.current = current;
                        obj.target = target;
                        obj.completed = current >= target;
                    }
                }
            }
        }
        "questCompleted" => {
            if let Some(value) = data {
                let quest_id = extract_string(value, "quest_id").unwrap_or_default();
                let quest_name = extract_string(value, "quest_name").unwrap_or_default();
                let exp_reward = extract_i32(value, "rewards_exp").unwrap_or(0);
                let gold_reward = extract_i32(value, "rewards_gold").unwrap_or(0);
                let completed_id = quest_id.clone();

                log::info!(
                    "Quest completed: {} - {} (EXP: {}, Gold: {})",
                    quest_id,
                    quest_name,
                    exp_reward,
                    gold_reward
                );

                // Add system chat message
                state.push_system_chat(format!("Quest '{}' complete!", quest_name));

                // Remove from active quests
                state.ui_state.active_quests.retain(|q| q.id != quest_id);

                // Play quest complete sound
                state.pending_sfx.push("quest_complete".to_string());

                // Add completion notification
                state
                    .ui_state
                    .quest_completed_events
                    .push(QuestCompletedEvent {
                        quest_id,
                        quest_name,
                        exp_reward,
                        gold_reward,
                        time: macroquad::time::get_time(),
                    });
                state.ui_state.completed_quest_ids.insert(completed_id);

                // Keep Adventurer Guide UI open and reset it after completion.
                if !reset_adventurer_guide_dialogue(state) {
                    clear_adventure_board_dialogue(state);
                    state.ui_state.active_dialogue = None;
                }
            }
        }
        "collectionLogDefinitions" => {
            if let Some(value) = data {
                if let Some(entries_val) = extract_map_field(value, "entries") {
                    if let rmpv::Value::Array(arr) = entries_val {
                        let mut defs = Vec::new();
                        for entry in arr {
                            if let rmpv::Value::Array(fields) = entry {
                                if fields.len() >= 3 {
                                    let item_id = fields[0].as_str().unwrap_or("").to_string();
                                    let source = fields[1].as_str().unwrap_or("").to_string();
                                    let source_detail =
                                        fields[2].as_str().unwrap_or("").to_string();
                                    defs.push((item_id, source, source_detail));
                                }
                            }
                        }
                        log::info!("Received {} collection log definitions", defs.len());
                        state.ui_state.collection_log_definitions = defs;
                    }
                }
                // Parse display names
                if let Some(names_val) = extract_map_field(value, "display_names") {
                    if let rmpv::Value::Array(arr) = names_val {
                        let mut names = std::collections::HashMap::new();
                        for entry in arr {
                            if let rmpv::Value::Array(fields) = entry {
                                if fields.len() >= 2 {
                                    let id = fields[0].as_str().unwrap_or("").to_string();
                                    let name = fields[1].as_str().unwrap_or("").to_string();
                                    names.insert(id, name);
                                }
                            }
                        }
                        log::info!("Received {} collection log display names", names.len());
                        state.ui_state.collection_log_display_names = names;
                    }
                }
            }
        }
        "collectionLogSync" => {
            if let Some(value) = data {
                if let Some(entries_val) = extract_map_field(value, "entries") {
                    if let rmpv::Value::Array(arr) = entries_val {
                        let mut log = std::collections::HashMap::new();
                        for entry in arr {
                            if let rmpv::Value::Array(fields) = entry {
                                if fields.len() >= 4 {
                                    let item_id = fields[0].as_str().unwrap_or("").to_string();
                                    let source = fields[1].as_str().unwrap_or("").to_string();
                                    let obtained_at = fields[3].as_str().unwrap_or("").to_string();
                                    log.insert((item_id, source), obtained_at);
                                }
                            }
                        }
                        log::info!("Synced {} collection log entries", log.len());
                        state.ui_state.collection_log = log;
                    }
                }
            }
        }
        "collectionLogEntry" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "item_id").unwrap_or_default();
                let source = extract_string(value, "source").unwrap_or_default();
                let obtained_at = extract_string(value, "obtained_at").unwrap_or_default();

                log::info!("New collection log entry: {} from {}", item_id, source);
                state
                    .ui_state
                    .collection_log
                    .insert((item_id.clone(), source), obtained_at);

                let display_name = state.item_registry.get_display_name(&item_id).to_string();
                state.push_system_chat(format!("New collection log entry: {}!", display_name));
                state.pending_sfx.push("enter".to_string());
            }
        }
        "dialogueClosed" => {
            // If we're in a port travel fade, transition to fade-in
            if matches!(
                state.map_transition.state,
                crate::game::state::TransitionState::FadingOut
                    | crate::game::state::TransitionState::Loading
            ) {
                state.map_transition.state = crate::game::state::TransitionState::FadingIn;
            }
            // Keep Adventurer Guide panel open and reset to its initial state.
            if !reset_adventurer_guide_dialogue(state) {
                clear_adventure_board_dialogue(state);
                state.ui_state.active_dialogue = None;
            }
        }

        // ========== Item Definition Messages ==========
        "itemDefinitions" => {
            if let Some(value) = data {
                let mut items = Vec::new();

                if let Some(items_arr) = extract_array(value, "items") {
                    for item_value in items_arr {
                        let id = extract_string(item_value, "id").unwrap_or_default();
                        let display_name =
                            extract_string(item_value, "displayName").unwrap_or_default();
                        let sprite = extract_string(item_value, "sprite").unwrap_or_default();
                        let category = extract_string(item_value, "category")
                            .unwrap_or_else(|| "material".to_string());
                        let max_stack = extract_i32(item_value, "maxStack").unwrap_or(99);
                        let description =
                            extract_string(item_value, "description").unwrap_or_default();
                        let base_price = extract_i32(item_value, "basePrice").unwrap_or(0);
                        let sellable = extract_bool(item_value, "sellable").unwrap_or(false);

                        // Parse equipment stats if present
                        let equipment =
                            extract_string(item_value, "equipment_slot").map(|slot_type| {
                                let chop_speed =
                                    extract_f32(item_value, "chop_speed_multiplier").unwrap_or(0.0);
                                if chop_speed > 0.0 {
                                    log::info!(
                                        "Loaded item {} with chop_speed_multiplier={}",
                                        id,
                                        chop_speed
                                    );
                                }
                                let mine_speed =
                                    extract_f32(item_value, "mine_speed_multiplier").unwrap_or(0.0);
                                if mine_speed > 0.0 {
                                    log::info!(
                                        "Loaded item {} with mine_speed_multiplier={}",
                                        id,
                                        mine_speed
                                    );
                                }
                                EquipmentStats {
                                    slot_type,
                                    attack_level_required: extract_i32(
                                        item_value,
                                        "attack_level_required",
                                    )
                                    .unwrap_or(1),
                                    defence_level_required: extract_i32(
                                        item_value,
                                        "defence_level_required",
                                    )
                                    .unwrap_or(1),
                                    ranged_level_required: extract_i32(
                                        item_value,
                                        "ranged_level_required",
                                    )
                                    .unwrap_or(0),
                                    attack_bonus: extract_i32(item_value, "attack_bonus")
                                        .unwrap_or(0),
                                    strength_bonus: extract_i32(item_value, "strength_bonus")
                                        .unwrap_or(0),
                                    defence_bonus: extract_i32(item_value, "defence_bonus")
                                        .unwrap_or(0),
                                    magic_bonus: extract_i32(item_value, "magic_bonus")
                                        .unwrap_or(0),
                                    magic_level_required: extract_i32(
                                        item_value,
                                        "magic_level_required",
                                    )
                                    .unwrap_or(0),
                                    woodcutting_level_required: extract_i32(
                                        item_value,
                                        "woodcutting_level_required",
                                    )
                                    .unwrap_or(1),
                                    chop_speed_multiplier: chop_speed,
                                    mining_level_required: extract_i32(
                                        item_value,
                                        "mining_level_required",
                                    )
                                    .unwrap_or(1),
                                    mine_speed_multiplier: mine_speed,
                                    ranged_strength_bonus: extract_i32(
                                        item_value,
                                        "ranged_strength_bonus",
                                    )
                                    .unwrap_or(0),
                                }
                            });

                        // Parse weapon fields
                        let weapon_type = extract_string(item_value, "weapon_type");
                        let range = extract_i32(item_value, "range");

                        items.push(ItemDefinition {
                            id,
                            display_name,
                            sprite,
                            category,
                            max_stack,
                            description,
                            base_price,
                            sellable,
                            equipment,
                            weapon_type,
                            range,
                            prayer_xp: extract_i32(item_value, "prayer_xp").unwrap_or(0),
                            ranged_strength: extract_i32(item_value, "ranged_strength")
                                .unwrap_or(0),
                            use_effect: extract_string(item_value, "use_effect_type"),
                        });
                    }
                }

                state.item_registry.load_from_server(items);
            }
        }

        // ========== Crafting System Messages ==========
        "recipeDefinitions" => {
            if let Some(value) = data {
                state.recipe_definitions.clear();

                if let Some(recipes_arr) = extract_array(value, "recipes") {
                    for recipe_value in recipes_arr {
                        let id = extract_string(recipe_value, "id").unwrap_or_default();
                        let display_name =
                            extract_string(recipe_value, "display_name").unwrap_or_default();
                        let description =
                            extract_string(recipe_value, "description").unwrap_or_default();
                        let category = extract_string(recipe_value, "category")
                            .unwrap_or_else(|| "consumables".to_string());
                        let section = extract_string(recipe_value, "section");
                        let level_required =
                            extract_i32(recipe_value, "level_required").unwrap_or(1);
                        let station = extract_string(recipe_value, "station");
                        let craft_time_ms = extract_u64(recipe_value, "craft_time_ms").unwrap_or(0);
                        let xp = extract_u32(recipe_value, "xp").unwrap_or(0);
                        let requires_discovery =
                            extract_bool(recipe_value, "requires_discovery").unwrap_or(false);
                        let required_tool = extract_string(recipe_value, "required_tool");
                        let burn_result = extract_string(recipe_value, "burn_result");
                        let burn_stop_level = extract_i32(recipe_value, "burn_stop_level");

                        // Parse ingredients
                        let mut ingredients = Vec::new();
                        if let Some(ing_arr) = extract_array(recipe_value, "ingredients") {
                            for ing_value in ing_arr {
                                let item_id =
                                    extract_string(ing_value, "item_id").unwrap_or_default();
                                let item_name =
                                    extract_string(ing_value, "item_name").unwrap_or_default();
                                let count = extract_i32(ing_value, "count").unwrap_or(1);
                                ingredients.push(RecipeIngredient {
                                    item_id,
                                    item_name,
                                    count,
                                });
                            }
                        }

                        // Parse results
                        let mut results = Vec::new();
                        if let Some(res_arr) = extract_array(recipe_value, "results") {
                            for res_value in res_arr {
                                let item_id =
                                    extract_string(res_value, "item_id").unwrap_or_default();
                                let item_name =
                                    extract_string(res_value, "item_name").unwrap_or_default();
                                let count = extract_i32(res_value, "count").unwrap_or(1);
                                results.push(RecipeResult {
                                    item_id,
                                    item_name,
                                    count,
                                });
                            }
                        }

                        state.recipe_definitions.push(RecipeDefinition {
                            id,
                            display_name,
                            description,
                            category,
                            section,
                            level_required,
                            ingredients,
                            results,
                            station,
                            craft_time_ms,
                            xp,
                            requires_discovery,
                            required_tool,
                            burn_result,
                            burn_stop_level,
                        });
                    }
                }

                log::info!(
                    "Received {} recipe definitions",
                    state.recipe_definitions.len()
                );
            }
        }
        _ => return false,
    }
    true
}
