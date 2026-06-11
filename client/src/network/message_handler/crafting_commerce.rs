use super::*;

pub(super) fn handle(msg_type: &str, data: Option<&rmpv::Value>, state: &mut GameState) -> bool {
    match msg_type {
        "shopOpen" => {
            if let Some(value) = data {
                let npc_id = extract_string(value, "npc_id").unwrap_or_default();
                log::info!("Opening shop for NPC: {}", npc_id);

                state.ui_state.crafting_open = true;
                state.ui_state.crafting_npc_id = Some(npc_id);
                state.ui_state.crafting_selected_category = 0;
                state.ui_state.crafting_selected_recipe = 0;
            }
        }
        "craftResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let error = extract_string(value, "error");

                if success {
                    log::info!("Crafting success: {}", recipe_id);
                    // Inventory update will come separately
                } else {
                    log::warn!("Crafting failed: {} - {:?}", recipe_id, error);
                    if let Some(err) = error {
                        state.push_system_chat(format!("Crafting failed: {}", err));
                    }
                }
            }
        }
        "discoveredRecipes" => {
            if let Some(value) = data {
                if let Some(recipes_arr) = extract_array(value, "recipes") {
                    state.discovered_recipes.clear();
                    for recipe_value in recipes_arr {
                        if let Some(recipe_id) = recipe_value.as_str() {
                            state.discovered_recipes.insert(recipe_id.to_string());
                        }
                    }
                    log::info!(
                        "Received {} discovered recipes",
                        state.discovered_recipes.len()
                    );
                }
            }
        }
        "recipeDiscovered" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                if !recipe_id.is_empty() {
                    state.discovered_recipes.insert(recipe_id.clone());

                    // Look up display name from recipe definitions
                    let display_name = state
                        .recipe_definitions
                        .iter()
                        .find(|r| r.id == recipe_id)
                        .map(|r| r.display_name.clone())
                        .unwrap_or_else(|| recipe_id.clone());

                    state.push_system_chat(format!("Recipe learned: {}", display_name));
                    log::info!("Recipe discovered: {}", recipe_id);
                }
            }
        }
        "craftingStarted" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let duration_ms = extract_u64(value, "duration_ms").unwrap_or(0);

                log::info!("Crafting started: {} ({}ms)", recipe_id, duration_ms);
                state.ui_state.crafting_in_progress = true;
                state.ui_state.crafting_recipe_id = Some(recipe_id);
                state.ui_state.crafting_duration_ms = duration_ms;
                state.ui_state.crafting_started_at = Some(macroquad::time::get_time());
                state.ui_state.crafting_progress = 0.0;
            }
        }
        "craftingCancelled" => {
            if let Some(value) = data {
                let reason = extract_string(value, "reason").unwrap_or_default();

                log::info!("Crafting cancelled: {}", reason);
                state.ui_state.crafting_in_progress = false;
                state.ui_state.crafting_recipe_id = None;
                state.ui_state.crafting_started_at = None;
                state.ui_state.crafting_progress = 0.0;

                if !reason.is_empty() {
                    state.push_system_chat(format!("Crafting cancelled: {}", reason));
                }
            }
        }
        "craftingCompleted" => {
            if let Some(value) = data {
                let recipe_id = extract_string(value, "recipe_id").unwrap_or_default();
                let xp_gained = extract_u32(value, "xp_gained").unwrap_or(0);

                log::info!("Crafting completed: {} (+{}xp)", recipe_id, xp_gained);

                // Clear crafting progress state
                state.ui_state.crafting_in_progress = false;
                state.ui_state.crafting_recipe_id = None;
                state.ui_state.crafting_started_at = None;
                state.ui_state.crafting_progress = 0.0;

                // Trigger completion animation (starts at 0.0, ticks up to 1.0)
                state.ui_state.crafting_complete_animation = Some((recipe_id.clone(), 0.0));

                // Look up display name from recipe definitions
                let display_name = state
                    .recipe_definitions
                    .iter()
                    .find(|r| r.id == recipe_id)
                    .map(|r| r.display_name.clone())
                    .unwrap_or_else(|| recipe_id.clone());

                let station = state
                    .recipe_definitions
                    .iter()
                    .find(|r| r.id == recipe_id)
                    .and_then(|r| r.station.as_deref())
                    .map(|s| s.to_string());

                if state.ui_state.batch_total > 1 {
                    // batch_completed hasn't been updated yet (batchProgress arrives after),
                    // so +1 to show 1-based count
                    state.push_system_chat(format!(
                        "{} ({}/{})",
                        display_name,
                        state.ui_state.batch_completed + 1,
                        state.ui_state.batch_total
                    ));
                } else {
                    let verb = match station.as_deref() {
                        Some("furnace") => "Smelted",
                        Some("alchemy_station") => "Brewed",
                        Some("fire_pit") => "Cooked",
                        _ => "Crafted",
                    };
                    state.push_system_chat(format!("{}: {}", verb, display_name));
                }

                // Play furnace sound on successful smelt/craft
                state.pending_sfx.push("furnace".to_string());

                // Inventory update and XP will come via separate messages
            }
        }
        "craftingBatchProgress" => {
            if let Some(value) = data {
                let completed = extract_u32(value, "completed").unwrap_or(0);
                let total = extract_u32(value, "total").unwrap_or(0);
                state.ui_state.batch_completed = completed;
                state.ui_state.batch_total = total;
                log::info!("Batch progress: {}/{}", completed, total);
            }
        }

        // ========== Equipment Messages ==========
        "equipmentUpdate" => {
            if let Some(value) = data {
                let player_id = extract_string(value, "player_id").unwrap_or_default();
                let equipped_head =
                    extract_string(value, "equipped_head").filter(|s| !s.is_empty());
                let equipped_body =
                    extract_string(value, "equipped_body").filter(|s| !s.is_empty());
                let equipped_weapon =
                    extract_string(value, "equipped_weapon").filter(|s| !s.is_empty());
                let equipped_back =
                    extract_string(value, "equipped_back").filter(|s| !s.is_empty());
                let equipped_feet =
                    extract_string(value, "equipped_feet").filter(|s| !s.is_empty());
                let equipped_ring =
                    extract_string(value, "equipped_ring").filter(|s| !s.is_empty());
                let equipped_gloves =
                    extract_string(value, "equipped_gloves").filter(|s| !s.is_empty());
                let equipped_necklace =
                    extract_string(value, "equipped_necklace").filter(|s| !s.is_empty());
                let equipped_belt =
                    extract_string(value, "equipped_belt").filter(|s| !s.is_empty());

                if let Some(player) = state.players.get_mut(&player_id) {
                    player.equipped_head = equipped_head.clone();
                    player.equipped_body = equipped_body.clone();
                    player.equipped_weapon = equipped_weapon.clone();
                    player.equipped_back = equipped_back.clone();
                    player.equipped_feet = equipped_feet.clone();
                    player.equipped_ring = equipped_ring.clone();
                    player.equipped_gloves = equipped_gloves.clone();
                    player.equipped_necklace = equipped_necklace.clone();
                    player.equipped_belt = equipped_belt.clone();
                    log::info!("Player {} equipment updated", player_id);
                }
            }
        }
        "equipResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let slot_type = extract_string(value, "slot_type").unwrap_or_default();
                let item_id = extract_string(value, "item_id");
                let error = extract_string(value, "error");

                if success {
                    log::info!("Equipment {} success: {:?}", slot_type, item_id);
                } else {
                    log::warn!("Equipment {} failed: {:?}", slot_type, error);
                    // TODO: Show error message in UI
                }
            }
        }

        // ========== Admin Messages ==========
        "announcement" => {
            if let Some(value) = data {
                let text = extract_string(value, "text").unwrap_or_default();
                log::info!("Server announcement: {}", text);
                state
                    .ui_state
                    .announcements
                    .push(crate::game::Announcement {
                        text,
                        time: macroquad::time::get_time(),
                    });
                state.pending_sfx.push("announce".to_string());
            }
        }

        // ========== Shop System Messages ==========

        // ========== Bank System Messages ==========
        "bankOpen" => {
            if let Some(value) = data {
                let mut slots = Vec::new();
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 {
                            slots.push((slot, item_id, quantity));
                        }
                    }
                }
                let gold = extract_i32(value, "gold").unwrap_or(0);
                let max_slots = extract_i32(value, "max_slots").unwrap_or(48) as u32;

                log::info!(
                    "Bank opened: {} items, {}g, {} max slots",
                    slots.len(),
                    gold,
                    max_slots
                );
                state.ui_state.bank_open = true;
                state.ui_state.bank_slots = vec![None; max_slots as usize];
                for (slot, item_id, quantity) in slots {
                    if (slot as usize) < state.ui_state.bank_slots.len() {
                        state.ui_state.bank_slots[slot as usize] = Some((item_id, quantity));
                    }
                }
                state.ui_state.bank_gold = gold;
                state.ui_state.bank_max_slots = max_slots;
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "bankUpdate" => {
            if let Some(value) = data {
                // Rebuild slots from server data
                let mut new_slots = vec![None; state.ui_state.bank_max_slots as usize];
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 && (slot as usize) < new_slots.len()
                        {
                            new_slots[slot as usize] = Some((item_id, quantity));
                        }
                    }
                }
                state.ui_state.bank_slots = new_slots;
                state.ui_state.bank_gold =
                    extract_i32(value, "gold").unwrap_or(state.ui_state.bank_gold);
            }
        }
        "bankResult" => {
            if let Some(value) = data {
                let success = extract_bool(value, "success").unwrap_or(false);
                let error = extract_string(value, "error");

                if !success {
                    if let Some(err) = error {
                        state.push_system_chat(format!("Bank: {}", err));
                    }
                }
            }
        }
        "chestOpen" => {
            if let Some(value) = data {
                let chest_id = extract_string(value, "chest_id").unwrap_or_default();
                let chest_name =
                    extract_string(value, "name").unwrap_or_else(|| "Chest".to_string());
                let total_value = extract_i32(value, "total_value").unwrap_or(0);
                let mut slots = Vec::new();
                if let Some(slots_arr) = extract_array(value, "slots") {
                    for slot_value in slots_arr {
                        let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                        let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                        let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                        let value = extract_i32(slot_value, "value").unwrap_or(0);
                        if !item_id.is_empty() && quantity > 0 {
                            slots.push((slot, item_id, quantity, value));
                        }
                    }
                }

                log::info!(
                    "Chest opened: '{}', {} items, total value {}g",
                    chest_id,
                    slots.len(),
                    total_value
                );

                // Determine slot count from the max slot index
                let max_slot = slots
                    .iter()
                    .map(|(s, _, _, _)| *s as usize)
                    .max()
                    .unwrap_or(0);
                let num_slots = (max_slot + 1).max(10);

                state.ui_state.chest_open = true;
                state.ui_state.inventory_open = true;
                state.ui_state.chest_id = chest_id;
                state.ui_state.chest_name = chest_name;
                state.ui_state.chest_slots = vec![None; num_slots];
                for (slot, item_id, quantity, value) in slots {
                    if (slot as usize) < state.ui_state.chest_slots.len() {
                        state.ui_state.chest_slots[slot as usize] =
                            Some((item_id, quantity, value));
                    }
                }
                state.ui_state.chest_total_value = total_value;
                state.ui_state.chest_scroll = 0.0;
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "chestUpdate" => {
            if let Some(value) = data {
                let chest_id = extract_string(value, "chest_id").unwrap_or_default();
                if chest_id == state.ui_state.chest_id {
                    let total_value = extract_i32(value, "total_value").unwrap_or(0);
                    let mut new_slots = vec![None; state.ui_state.chest_slots.len()];
                    if let Some(slots_arr) = extract_array(value, "slots") {
                        for slot_value in slots_arr {
                            let slot = extract_i32(slot_value, "slot").unwrap_or(0) as u8;
                            let item_id = extract_string(slot_value, "item_id").unwrap_or_default();
                            let quantity = extract_i32(slot_value, "quantity").unwrap_or(0);
                            let value = extract_i32(slot_value, "value").unwrap_or(0);
                            if !item_id.is_empty()
                                && quantity > 0
                                && (slot as usize) < new_slots.len()
                            {
                                new_slots[slot as usize] = Some((item_id, quantity, value));
                            }
                        }
                    }
                    state.ui_state.chest_slots = new_slots;
                    state.ui_state.chest_total_value = total_value;
                }
            }
        }
        "shopData" => {
            if let Some(value) = data {
                // Extract npcId from top level (camelCase from server)
                let npc_id = extract_string(value, "npcId").unwrap_or_default();

                // Extract shop data from nested "shop" field
                let shop_value = value
                    .as_map()
                    .and_then(|m| {
                        m.iter()
                            .find(|(k, _)| k.as_str() == Some("shop"))
                            .map(|(_, v)| v)
                    })
                    .unwrap_or(value);

                let shop_id = extract_string(shop_value, "shopId").unwrap_or_default();
                let display_name =
                    extract_string(shop_value, "displayName").unwrap_or_else(|| "Shop".to_string());
                let buy_multiplier = extract_f32(shop_value, "buyMultiplier").unwrap_or(0.5);
                let sell_multiplier = extract_f32(shop_value, "sellMultiplier").unwrap_or(1.0);

                // Parse crafting categories from server
                let crafting_categories: Vec<String> =
                    extract_array(shop_value, "craftingCategories")
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                let crafting_stations: Vec<String> = extract_array(shop_value, "craftingStations")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let show_crafting = !crafting_categories.is_empty();

                let mut stock = Vec::new();
                if let Some(stock_arr) = extract_array(shop_value, "stock") {
                    for item_value in stock_arr {
                        let item_id = extract_string(item_value, "itemId").unwrap_or_default();
                        let quantity = extract_i32(item_value, "quantity").unwrap_or(0);
                        let price = extract_i32(item_value, "price").unwrap_or(0);

                        stock.push(ShopStockItem {
                            item_id,
                            quantity,
                            price,
                        });
                    }
                }

                log::info!(
                    "Shop data received: {} items from {} (npc: {})",
                    stock.len(),
                    display_name,
                    npc_id
                );
                state.ui_state.shop_npc_id = Some(npc_id);
                state.ui_state.shop_data = Some(ShopData {
                    shop_id,
                    display_name,
                    buy_multiplier,
                    sell_multiplier,
                    show_crafting,
                    crafting_categories,
                    crafting_stations,
                    stock,
                });
                state.ui_state.crafting_open = true; // Open crafting window (which has shop tab)
                state.ui_state.shop_main_tab = 1; // Switch to Shop tab
                state.pending_sfx.push("ui_open".to_string());
            }
        }
        "shopResult" => {
            if let Some(value) = data {
                let success = value
                    .as_map()
                    .and_then(|map| map.iter().find(|(k, _)| k.as_str() == Some("success")))
                    .and_then(|(_, v)| v.as_bool())
                    .unwrap_or(false);
                let action = extract_string(value, "action").unwrap_or_default();
                let item_id = extract_string(value, "itemId").unwrap_or_default();
                let quantity = extract_i32(value, "quantity").unwrap_or(0);
                let gold_change = extract_i32(value, "goldChange").unwrap_or(0);
                let error = extract_string(value, "error");

                if success {
                    log::info!("Shop transaction successful");

                    // Get item display name from registry
                    let item_name = state
                        .item_registry
                        .get(&item_id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| item_id.clone());

                    // Add system chat message
                    let message = if action == "buy" {
                        format!(
                            "Bought {}x {} for {}g",
                            quantity,
                            item_name,
                            gold_change.abs()
                        )
                    } else {
                        format!(
                            "Sold {}x {} for {}g",
                            quantity,
                            item_name,
                            gold_change.abs()
                        )
                    };
                    state.push_system_chat(message);
                } else if let Some(err) = error {
                    log::warn!("Shop transaction failed: {}", err);
                    // Show error in system chat
                    state.push_system_chat(format!("Transaction failed: {}", err));
                }
            }
        }
        "shopStockUpdate" => {
            if let Some(value) = data {
                let item_id = extract_string(value, "itemId").unwrap_or_default();
                let new_quantity = extract_i32(value, "newQuantity").unwrap_or(0);

                // Update the stock in the current shop data if it's open
                if let Some(shop_data) = &mut state.ui_state.shop_data {
                    if let Some(item) = shop_data.stock.iter_mut().find(|i| i.item_id == item_id) {
                        item.quantity = new_quantity;
                        log::debug!(
                            "Shop stock updated: {} now has {} in stock",
                            item_id,
                            new_quantity
                        );
                    }
                }
            }
        }
        _ => return false,
    }
    true
}
