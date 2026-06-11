use super::*;
use crate::crafting::definition::{RecipeCategory, RecipeDefinition};
use crate::protocol::RecipeResult as ProtoRecipeResult;

type CraftXpUpdate = (SkillType, i64, i64, i32, bool);

struct TimedCraftCompletion {
    pid: String,
    recipe_id: String,
    items_gained: Vec<(String, u32)>,
    xp_gained: u32,
    inventory_update: ServerMessage,
    xp_results: Vec<CraftXpUpdate>,
    batch_continued: bool,
    next_duration_ms: Option<u64>,
    batch_completed: u32,
    batch_total: u32,
    burned: bool,
    category: RecipeCategory,
}

fn recipe_level_check_passed(player: &Player, recipe: &RecipeDefinition) -> bool {
    match recipe.category {
        RecipeCategory::Smithing => player.skills.smithing.level >= recipe.level_required,
        RecipeCategory::Alchemy => player.skills.alchemy.level >= recipe.level_required,
        RecipeCategory::Cooking | RecipeCategory::Fletching | RecipeCategory::Leatherworking => {
            player.skills.survivalist.level >= recipe.level_required
        }
        _ => player.combat_level() >= recipe.level_required,
    }
}

fn recipe_skill_name(category: RecipeCategory) -> &'static str {
    match category {
        RecipeCategory::Smithing => "Smithing",
        RecipeCategory::Alchemy => "Alchemy",
        RecipeCategory::Cooking | RecipeCategory::Fletching | RecipeCategory::Leatherworking => {
            "Survivalist"
        }
        _ => "Combat",
    }
}

fn collection_log_skill(category: RecipeCategory) -> &'static str {
    match category {
        RecipeCategory::Smithing => "smithing",
        RecipeCategory::Alchemy => "alchemy",
        RecipeCategory::Cooking => "cooking",
        RecipeCategory::Fletching => "fletching",
        RecipeCategory::Leatherworking => "leatherworking",
        _ => "crafting",
    }
}

fn inventory_update_message(player_id: &str, player: &Player) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        player_id: player_id.to_string(),
        slots: player.inventory.to_update(),
        gold: player.inventory.gold,
    }
}

fn can_continue_batch(
    player: &Player,
    recipe: &RecipeDefinition,
    item_registry: &ItemRegistry,
) -> bool {
    let has_ingredients = recipe.ingredients.iter().all(|ingredient| {
        player
            .inventory
            .has_item(&ingredient.item_id, ingredient.count)
    });
    let has_space = recipe.results.iter().all(|result| {
        player
            .inventory
            .has_space_for(&result.item_id, result.count, item_registry)
    });

    has_ingredients && has_space
}

fn add_crafting_xp(
    player: &mut Player,
    category: RecipeCategory,
    xp_gained: u32,
) -> Vec<CraftXpUpdate> {
    let mut xp_results = Vec::new();
    if xp_gained == 0 {
        return xp_results;
    }

    let xp_gained = xp_gained as i64;
    if category == RecipeCategory::Smithing {
        let leveled = player.skills.smithing.add_xp(xp_gained);
        xp_results.push((
            SkillType::Smithing,
            xp_gained,
            player.skills.smithing.xp,
            player.skills.smithing.level,
            leveled,
        ));
    }
    if category == RecipeCategory::Alchemy {
        let leveled = player.skills.alchemy.add_xp(xp_gained);
        xp_results.push((
            SkillType::Alchemy,
            xp_gained,
            player.skills.alchemy.xp,
            player.skills.alchemy.level,
            leveled,
        ));
    }
    if matches!(
        category,
        RecipeCategory::Cooking | RecipeCategory::Fletching | RecipeCategory::Leatherworking
    ) {
        let leveled = player.skills.survivalist.add_xp(xp_gained);
        xp_results.push((
            SkillType::Survivalist,
            xp_gained,
            player.skills.survivalist.xp,
            player.skills.survivalist.level,
            leveled,
        ));
    }

    xp_results
}

impl GameRoom {
    async fn can_use_recipe_station(&self, player_id: &str, recipe: &RecipeDefinition) -> bool {
        let Some(required_station) = recipe.station.as_deref() else {
            return true;
        };
        let Some((_npc_id, prototype_id)) =
            self.validate_active_npc_interaction(player_id, 2.5).await
        else {
            return false;
        };
        self.entity_registry
            .get(&prototype_id)
            .is_some_and(|prototype| {
                prototype.behaviors.station_type.as_deref() == Some(required_station)
                    || prototype.merchant.as_ref().is_some_and(|merchant| {
                        merchant
                            .crafting_stations
                            .iter()
                            .any(|station| station == required_station)
                    })
            })
    }

    async fn send_crafting_xp_updates(&self, player_id: &str, xp_results: Vec<CraftXpUpdate>) {
        let mut any_leveled = false;
        for (skill_type, xp_amount, total_xp, level, leveled_up) in xp_results {
            self.send_to_player(
                player_id,
                ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: skill_type.as_str().to_string(),
                    xp_gained: xp_amount,
                    total_xp,
                    level,
                },
            )
            .await;

            if leveled_up {
                tracing::info!(
                    "Player {} leveled up {} to {}",
                    player_id,
                    skill_type.as_str(),
                    level
                );
                self.broadcast_skill_level_up(player_id, skill_type.as_str(), level)
                    .await;
                any_leveled = true;
            }
        }

        if any_leveled {
            self.process_quest_progression_snapshot(player_id).await;
        }
    }

    pub(super) async fn handle_use_recipe_scroll(&self, player_id: &str, slot_index: u8) {
        let (item_id, recipe_id, inventory_update, gold) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };

            let item_id = match player
                .inventory
                .slots
                .get(slot_index as usize)
                .and_then(|s| s.as_ref())
            {
                Some(slot) => slot.item_id.clone(),
                None => return,
            };

            let recipe_id = match item_id.strip_prefix("recipe_") {
                Some(id) => id.to_string(),
                None => return,
            };

            if self.crafting_registry.get(&recipe_id).is_none() {
                drop(players);
                self.send_system_message(player_id, "This recipe scroll is for an unknown recipe.")
                    .await;
                return;
            }

            if player.discovered_recipes.contains(&recipe_id) {
                drop(players);
                self.send_system_message(player_id, "You already know this recipe.")
                    .await;
                return;
            }

            if let Some(ref mut slot) = player.inventory.slots[slot_index as usize] {
                slot.quantity -= 1;
                if slot.quantity <= 0 {
                    player.inventory.slots[slot_index as usize] = None;
                }
            }

            player.discovered_recipes.insert(recipe_id.clone());

            (
                item_id,
                recipe_id,
                player.inventory.to_update(),
                player.inventory.gold,
            )
        };

        let display_name = self
            .item_registry
            .get(&item_id)
            .map(|def| def.display_name.clone())
            .unwrap_or_else(|| item_id.clone());
        tracing::info!(
            "Player {} used recipe scroll {} -> discovered recipe {}",
            player_id,
            display_name,
            recipe_id
        );

        if let Some(ref db) = self.db {
            if let Some(character_id) = Self::parse_character_id(player_id) {
                if let Err(e) = db.save_discovered_recipe(character_id, &recipe_id).await {
                    tracing::warn!("Failed to save discovered recipe to DB: {}", e);
                }
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::RecipeDiscovered {
                recipe_id: recipe_id.clone(),
            },
        )
        .await;
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        )
        .await;
    }

    pub async fn handle_craft(&self, player_id: &str, recipe_id: &str) {
        let recipe = match self.crafting_registry.get(recipe_id) {
            Some(recipe) => recipe.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftResult {
                        success: false,
                        recipe_id: recipe_id.to_string(),
                        error: Some("Recipe not found".to_string()),
                        items_gained: vec![],
                    },
                )
                .await;
                return;
            }
        };
        if !self.can_use_recipe_station(player_id, &recipe).await {
            tracing::warn!(
                "Rejected remote crafting action from {} for recipe {}",
                player_id,
                recipe_id
            );
            return;
        }

        let (items_gained, inv_msg, xp_results, burned) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            if !recipe_level_check_passed(player, &recipe) {
                let skill_name = recipe_skill_name(recipe.category);
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftResult {
                        success: false,
                        recipe_id: recipe_id.to_string(),
                        error: Some(format!(
                            "Requires {} level {}",
                            skill_name, recipe.level_required
                        )),
                        items_gained: vec![],
                    },
                )
                .await;
                return;
            }

            if let Some(ref tool) = recipe.required_tool {
                if !player.inventory.has_item(tool, 1) {
                    let tool_name = self
                        .item_registry
                        .get(tool)
                        .map(|d| d.display_name.as_str())
                        .unwrap_or(tool.as_str());
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::CraftResult {
                            success: false,
                            recipe_id: recipe_id.to_string(),
                            error: Some(format!("You need a {} to do that", tool_name)),
                            items_gained: vec![],
                        },
                    )
                    .await;
                    return;
                }
            }

            for ingredient in &recipe.ingredients {
                if !player
                    .inventory
                    .has_item(&ingredient.item_id, ingredient.count)
                {
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::CraftResult {
                            success: false,
                            recipe_id: recipe_id.to_string(),
                            error: Some("Missing ingredients".to_string()),
                            items_gained: vec![],
                        },
                    )
                    .await;
                    return;
                }
            }

            for result in &recipe.results {
                if !player.inventory.has_space_for(
                    &result.item_id,
                    result.count,
                    &self.item_registry,
                ) {
                    drop(players);
                    self.send_to_player(
                        player_id,
                        ServerMessage::CraftResult {
                            success: false,
                            recipe_id: recipe_id.to_string(),
                            error: Some("Inventory full".to_string()),
                            items_gained: vec![],
                        },
                    )
                    .await;
                    return;
                }
            }

            for ingredient in &recipe.ingredients {
                player
                    .inventory
                    .remove_item(&ingredient.item_id, ingredient.count);
            }

            let burned = check_burn(&recipe, player.skills.survivalist.level);
            let mut items_gained = Vec::new();
            if burned {
                let burn_item = recipe.burn_result.as_ref().unwrap();
                player.inventory.add_item(burn_item, 1, &self.item_registry);
                let display_name = self
                    .item_registry
                    .get(burn_item)
                    .map(|def| def.display_name.clone())
                    .unwrap_or_else(|| burn_item.clone());
                items_gained.push(ProtoRecipeResult {
                    item_id: burn_item.clone(),
                    item_name: display_name,
                    count: 1,
                });
            } else {
                for result in &recipe.results {
                    player
                        .inventory
                        .add_item(&result.item_id, result.count, &self.item_registry);
                    let display_name = self
                        .item_registry
                        .get(&result.item_id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| result.item_id.clone());
                    items_gained.push(ProtoRecipeResult {
                        item_id: result.item_id.clone(),
                        item_name: display_name,
                        count: result.count,
                    });
                }
            }

            let xp_gained = if burned { recipe.xp / 2 } else { recipe.xp };
            let xp_results = add_crafting_xp(player, recipe.category, xp_gained);
            (
                items_gained,
                inventory_update_message(player_id, player),
                xp_results,
                burned,
            )
        };

        tracing::info!(
            "Player {} crafted {} (gained {:?})",
            player_id,
            recipe_id,
            items_gained
        );

        for result in &items_gained {
            self.record_resource_contract_progress(player_id, &result.item_id, result.count)
                .await;
        }

        if !burned {
            let skill = collection_log_skill(recipe.category);
            for result in &items_gained {
                self.record_collection_entry(player_id, &result.item_id, "skilling", skill)
                    .await;
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::CraftResult {
                success: true,
                recipe_id: recipe_id.to_string(),
                error: None,
                items_gained,
            },
        )
        .await;
        self.send_to_player(player_id, inv_msg).await;
        self.send_crafting_xp_updates(player_id, xp_results).await;
    }

    pub async fn handle_start_craft(&self, player_id: &str, recipe_id: &str) {
        let recipe = match self.crafting_registry.get(recipe_id) {
            Some(recipe) => recipe.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Recipe not found".to_string(),
                    },
                )
                .await;
                return;
            }
        };
        if !self.can_use_recipe_station(player_id, &recipe).await {
            tracing::warn!(
                "Rejected remote crafting action from {} for recipe {}",
                player_id,
                recipe_id
            );
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(player) if player.active && !player.is_dead => player,
            _ => return,
        };

        if player.crafting_state.is_some() {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Already crafting".to_string(),
                },
            )
            .await;
            return;
        }

        if !recipe_level_check_passed(player, &recipe) {
            let skill_name = recipe_skill_name(recipe.category);
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: format!("Requires {} level {}", skill_name, recipe.level_required),
                },
            )
            .await;
            return;
        }

        if let Some(ref tool) = recipe.required_tool {
            if !player.inventory.has_item(tool, 1) {
                let tool_name = self
                    .item_registry
                    .get(tool)
                    .map(|d| d.display_name.as_str())
                    .unwrap_or(tool.as_str());
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: format!("You need a {} to do that", tool_name),
                    },
                )
                .await;
                return;
            }
        }

        if recipe.requires_discovery && !player.discovered_recipes.contains(recipe_id) {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Recipe not yet discovered".to_string(),
                },
            )
            .await;
            return;
        }

        for ingredient in &recipe.ingredients {
            let have = player.inventory.count_item(&ingredient.item_id);
            if have < ingredient.count {
                tracing::warn!(
                    "Player {} craft {} failed: need {}x '{}' but have {}",
                    player_id,
                    recipe_id,
                    ingredient.count,
                    ingredient.item_id,
                    have
                );
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: format!(
                            "Missing ingredients: need {}x {} but have {}",
                            ingredient.count, ingredient.item_id, have
                        ),
                    },
                )
                .await;
                return;
            }
        }

        for result in &recipe.results {
            if !player
                .inventory
                .has_space_for(&result.item_id, result.count, &self.item_registry)
            {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Inventory full".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        let mut consumed_materials = Vec::new();
        for ingredient in &recipe.ingredients {
            player
                .inventory
                .remove_item(&ingredient.item_id, ingredient.count);
            consumed_materials.push((ingredient.item_id.clone(), ingredient.count));
        }

        if recipe.craft_time_ms == 0 {
            let burned = check_burn(&recipe, player.skills.survivalist.level);
            let mut items_gained = Vec::new();
            if burned {
                let burn_item = recipe.burn_result.as_ref().unwrap();
                player.inventory.add_item(burn_item, 1, &self.item_registry);
                items_gained.push((burn_item.clone(), 1));
            } else {
                for result in &recipe.results {
                    player
                        .inventory
                        .add_item(&result.item_id, result.count, &self.item_registry);
                    items_gained.push((result.item_id.clone(), result.count as u32));
                }
            }

            let xp_gained = if burned { recipe.xp / 2 } else { recipe.xp };
            let xp_results = add_crafting_xp(player, recipe.category, xp_gained);
            let inv_msg = inventory_update_message(player_id, player);
            drop(players);

            tracing::info!(
                "Player {} instant-crafted {} (gained {:?})",
                player_id,
                recipe_id,
                items_gained
            );

            self.send_to_player(
                player_id,
                ServerMessage::CraftingCompleted {
                    recipe_id: recipe_id.to_string(),
                    items_gained: items_gained.clone(),
                    xp_gained,
                },
            )
            .await;

            for (item_id_gained, count) in &items_gained {
                self.process_quest_item_collect(player_id, item_id_gained, *count as i32)
                    .await;
                self.record_resource_contract_progress(player_id, item_id_gained, *count as i32)
                    .await;
            }

            if !burned {
                let skill = collection_log_skill(recipe.category);
                for (item_id_gained, _count) in &items_gained {
                    self.record_collection_entry(player_id, item_id_gained, "skilling", skill)
                        .await;
                }
            }

            self.send_to_player(player_id, inv_msg).await;
            self.send_crafting_xp_updates(player_id, xp_results).await;
            return;
        }

        player.crafting_state = Some(CraftingState {
            recipe_id: recipe_id.to_string(),
            started_at: std::time::Instant::now(),
            duration_ms: recipe.craft_time_ms,
            consumed_materials,
            batch_remaining: 0,
            batch_total: 1,
        });

        let inv_msg = inventory_update_message(player_id, player);
        drop(players);

        tracing::info!(
            "Player {} started timed craft {} ({}ms)",
            player_id,
            recipe_id,
            recipe.craft_time_ms
        );

        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            ServerMessage::CraftingStarted {
                recipe_id: recipe_id.to_string(),
                duration_ms: recipe.craft_time_ms,
            },
        )
        .await;
    }

    pub async fn handle_cancel_craft(&self, player_id: &str) {
        self.cancel_crafting(player_id, "cancelled").await;
    }

    pub async fn cancel_crafting(&self, player_id: &str, reason: &str) {
        let refund_result = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(crafting) = player.crafting_state.take() {
                    for (item_id, count) in &crafting.consumed_materials {
                        player
                            .inventory
                            .add_item(item_id, *count, &self.item_registry);
                    }
                    Some(inventory_update_message(player_id, player))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(inv_msg) = refund_result {
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: reason.to_string(),
                },
            )
            .await;
            self.send_to_player(player_id, inv_msg).await;
        }
    }

    pub async fn process_timed_crafting_completions(&self) {
        let completions = {
            let mut players = self.players.write().await;
            let mut completions = Vec::new();

            let player_ids: Vec<String> = players.keys().cloned().collect();
            for pid in player_ids {
                let Some(player) = players.get_mut(&pid) else {
                    continue;
                };
                if !player.active || player.is_dead {
                    continue;
                }

                let should_complete = player.crafting_state.as_ref().is_some_and(|state| {
                    state.started_at.elapsed().as_millis() as u64 >= state.duration_ms
                });
                if !should_complete {
                    continue;
                }

                let crafting = player.crafting_state.take().unwrap();
                let recipe = match self.crafting_registry.get(&crafting.recipe_id) {
                    Some(recipe) => recipe.clone(),
                    None => continue,
                };

                let burned = check_burn(&recipe, player.skills.survivalist.level);
                let mut items_gained = Vec::new();
                if burned {
                    let burn_item = recipe.burn_result.as_ref().unwrap();
                    player.inventory.add_item(burn_item, 1, &self.item_registry);
                    items_gained.push((burn_item.clone(), 1));
                } else {
                    for result in &recipe.results {
                        player.inventory.add_item(
                            &result.item_id,
                            result.count,
                            &self.item_registry,
                        );
                        items_gained.push((result.item_id.clone(), result.count as u32));
                    }
                }

                let xp_gained = if burned { recipe.xp / 2 } else { recipe.xp };
                let xp_results = add_crafting_xp(player, recipe.category, xp_gained);

                let batch_total = crafting.batch_total;
                let completed_count = batch_total - crafting.batch_remaining;

                let mut next_duration_ms = None;
                let batch_continued = if crafting.batch_remaining > 0
                    && can_continue_batch(player, &recipe, &self.item_registry)
                {
                    let mut next_consumed = Vec::new();
                    for ingredient in &recipe.ingredients {
                        player
                            .inventory
                            .remove_item(&ingredient.item_id, ingredient.count);
                        next_consumed.push((ingredient.item_id.clone(), ingredient.count));
                    }
                    player.crafting_state = Some(CraftingState {
                        recipe_id: crafting.recipe_id.clone(),
                        started_at: std::time::Instant::now(),
                        duration_ms: recipe.craft_time_ms,
                        consumed_materials: next_consumed,
                        batch_remaining: crafting.batch_remaining - 1,
                        batch_total,
                    });
                    next_duration_ms = Some(recipe.craft_time_ms);
                    true
                } else {
                    false
                };

                completions.push(TimedCraftCompletion {
                    pid: pid.clone(),
                    recipe_id: crafting.recipe_id,
                    items_gained,
                    xp_gained,
                    inventory_update: inventory_update_message(&pid, player),
                    xp_results,
                    batch_continued,
                    next_duration_ms,
                    batch_completed: completed_count,
                    batch_total,
                    burned,
                    category: recipe.category,
                });
            }

            completions
        };

        for completion in completions {
            tracing::info!(
                "Player {} completed timed craft {} (gained {:?})",
                completion.pid,
                completion.recipe_id,
                completion.items_gained
            );

            self.send_to_player(
                &completion.pid,
                ServerMessage::CraftingCompleted {
                    recipe_id: completion.recipe_id.clone(),
                    items_gained: completion.items_gained.clone(),
                    xp_gained: completion.xp_gained,
                },
            )
            .await;

            for (item_id_gained, count) in &completion.items_gained {
                self.process_quest_item_collect(&completion.pid, item_id_gained, *count as i32)
                    .await;
                self.record_resource_contract_progress(
                    &completion.pid,
                    item_id_gained,
                    *count as i32,
                )
                .await;
            }

            if !completion.burned {
                let skill = collection_log_skill(completion.category);
                for (item_id_gained, _count) in &completion.items_gained {
                    self.record_collection_entry(
                        &completion.pid,
                        item_id_gained,
                        "skilling",
                        skill,
                    )
                    .await;
                }
            }

            self.send_to_player(&completion.pid, completion.inventory_update)
                .await;
            self.send_crafting_xp_updates(&completion.pid, completion.xp_results)
                .await;

            if completion.batch_continued {
                self.send_to_player(
                    &completion.pid,
                    ServerMessage::CraftingStarted {
                        recipe_id: completion.recipe_id,
                        duration_ms: completion.next_duration_ms.unwrap_or(2000),
                    },
                )
                .await;
                self.send_to_player(
                    &completion.pid,
                    ServerMessage::CraftingBatchProgress {
                        completed: completion.batch_completed,
                        total: completion.batch_total,
                    },
                )
                .await;
            } else if completion.batch_total > 1 {
                self.send_to_player(
                    &completion.pid,
                    ServerMessage::CraftingBatchProgress {
                        completed: completion.batch_completed,
                        total: completion.batch_total,
                    },
                )
                .await;
            }
        }
    }

    pub async fn handle_start_craft_batch(&self, player_id: &str, recipe_id: &str, quantity: u32) {
        let recipe = match self.crafting_registry.get(recipe_id) {
            Some(recipe) => recipe.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Recipe not found".to_string(),
                    },
                )
                .await;
                return;
            }
        };
        if !self.can_use_recipe_station(player_id, &recipe).await {
            tracing::warn!(
                "Rejected remote crafting action from {} for recipe {}",
                player_id,
                recipe_id
            );
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(player) if player.active && !player.is_dead => player,
            _ => return,
        };

        if player.crafting_state.is_some() {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Already crafting".to_string(),
                },
            )
            .await;
            return;
        }

        if !recipe_level_check_passed(player, &recipe) {
            let skill_name = recipe_skill_name(recipe.category);
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: format!("Requires {} level {}", skill_name, recipe.level_required),
                },
            )
            .await;
            return;
        }

        if let Some(ref tool) = recipe.required_tool {
            if !player.inventory.has_item(tool, 1) {
                let tool_name = self
                    .item_registry
                    .get(tool)
                    .map(|d| d.display_name.as_str())
                    .unwrap_or(tool.as_str());
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: format!("You need a {} to do that", tool_name),
                    },
                )
                .await;
                return;
            }
        }

        if recipe.requires_discovery && !player.discovered_recipes.contains(recipe_id) {
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::CraftingCancelled {
                    reason: "Recipe not yet discovered".to_string(),
                },
            )
            .await;
            return;
        }

        let actual_quantity = if quantity == u32::MAX {
            let mut max_possible = u32::MAX;
            for ingredient in &recipe.ingredients {
                let have = player.inventory.count_item(&ingredient.item_id) as u32;
                let can_make = have / ingredient.count as u32;
                max_possible = max_possible.min(can_make);
            }
            max_possible.max(1)
        } else {
            quantity.max(1)
        };

        for ingredient in &recipe.ingredients {
            if !player
                .inventory
                .has_item(&ingredient.item_id, ingredient.count)
            {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Missing ingredients".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        for result in &recipe.results {
            if !player
                .inventory
                .has_space_for(&result.item_id, result.count, &self.item_registry)
            {
                drop(players);
                self.send_to_player(
                    player_id,
                    ServerMessage::CraftingCancelled {
                        reason: "Inventory full".to_string(),
                    },
                )
                .await;
                return;
            }
        }

        let mut consumed_materials = Vec::new();
        for ingredient in &recipe.ingredients {
            player
                .inventory
                .remove_item(&ingredient.item_id, ingredient.count);
            consumed_materials.push((ingredient.item_id.clone(), ingredient.count));
        }

        player.crafting_state = Some(CraftingState {
            recipe_id: recipe_id.to_string(),
            started_at: std::time::Instant::now(),
            duration_ms: recipe.craft_time_ms,
            consumed_materials,
            batch_remaining: actual_quantity - 1,
            batch_total: actual_quantity,
        });

        let inv_msg = inventory_update_message(player_id, player);
        drop(players);

        tracing::info!(
            "Player {} started batch craft {} x{} ({}ms each)",
            player_id,
            recipe_id,
            actual_quantity,
            recipe.craft_time_ms
        );

        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            ServerMessage::CraftingStarted {
                recipe_id: recipe_id.to_string(),
                duration_ms: recipe.craft_time_ms,
            },
        )
        .await;
        self.send_to_player(
            player_id,
            ServerMessage::CraftingBatchProgress {
                completed: 0,
                total: actual_quantity,
            },
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_recipe(category: RecipeCategory, level_required: i32) -> RecipeDefinition {
        RecipeDefinition {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            description: String::new(),
            category,
            section: None,
            level_required,
            ingredients: vec![],
            results: vec![],
            station: None,
            craft_time_ms: 0,
            xp: 0,
            requires_discovery: false,
            required_tool: None,
            burn_result: None,
            burn_stop_level: None,
        }
    }

    #[test]
    fn recipe_level_check_passed_routes_to_expected_skill_bucket() {
        let mut player = Player::new("char_1", "Tester", 0, 0, "m", "skin1", None, None);
        player.skills.smithing.level = 9;
        player.skills.alchemy.level = 7;
        player.skills.survivalist.level = 5;
        player.skills.attack.level = 20;
        player.skills.strength.level = 20;
        player.skills.defence.level = 20;
        player.skills.hitpoints.level = 20;

        assert!(recipe_level_check_passed(
            &player,
            &test_recipe(RecipeCategory::Smithing, 9)
        ));
        assert!(!recipe_level_check_passed(
            &player,
            &test_recipe(RecipeCategory::Smithing, 10)
        ));
        assert!(recipe_level_check_passed(
            &player,
            &test_recipe(RecipeCategory::Alchemy, 7)
        ));
        assert!(recipe_level_check_passed(
            &player,
            &test_recipe(RecipeCategory::Cooking, 5)
        ));
        assert!(recipe_level_check_passed(
            &player,
            &test_recipe(RecipeCategory::Materials, 10)
        ));
    }

    #[test]
    fn add_crafting_xp_updates_only_the_matching_skill_track() {
        let mut player = Player::new("char_1", "Tester", 0, 0, "m", "skin1", None, None);

        let smithing = add_crafting_xp(&mut player, RecipeCategory::Smithing, 50);
        assert_eq!(smithing.len(), 1);
        assert_eq!(smithing[0].0, SkillType::Smithing);
        assert!(player.skills.smithing.xp > 0);
        assert_eq!(player.skills.alchemy.xp, 0);
        assert_eq!(player.skills.survivalist.xp, 0);

        let survivalist = add_crafting_xp(&mut player, RecipeCategory::Cooking, 30);
        assert_eq!(survivalist.len(), 1);
        assert_eq!(survivalist[0].0, SkillType::Survivalist);
        assert!(player.skills.survivalist.xp > 0);

        let none = add_crafting_xp(&mut player, RecipeCategory::Materials, 30);
        assert!(none.is_empty());
    }

    #[test]
    fn recipe_skill_name_matches_category_groups() {
        assert_eq!(recipe_skill_name(RecipeCategory::Smithing), "Smithing");
        assert_eq!(recipe_skill_name(RecipeCategory::Alchemy), "Alchemy");
        assert_eq!(recipe_skill_name(RecipeCategory::Cooking), "Survivalist");
        assert_eq!(recipe_skill_name(RecipeCategory::Materials), "Combat");
    }

    #[test]
    fn can_continue_batch_requires_ingredients_and_result_space() {
        let registry = ItemRegistry::default();
        let recipe = RecipeDefinition {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            description: String::new(),
            category: RecipeCategory::Smithing,
            section: None,
            level_required: 1,
            ingredients: vec![crate::crafting::definition::Ingredient {
                item_id: "ore".to_string(),
                count: 1,
            }],
            results: vec![crate::crafting::definition::CraftResult {
                item_id: "bar".to_string(),
                count: 1,
            }],
            station: None,
            craft_time_ms: 1000,
            xp: 5,
            requires_discovery: false,
            required_tool: None,
            burn_result: None,
            burn_stop_level: None,
        };

        let mut player = Player::new("char_1", "Tester", 0, 0, "m", "skin1", None, None);
        player.inventory.add_item("ore", 2, &registry);
        assert!(can_continue_batch(&player, &recipe, &registry));

        let mut missing_ingredients =
            Player::new("char_2", "Tester", 0, 0, "m", "skin1", None, None);
        assert!(!can_continue_batch(
            &missing_ingredients,
            &recipe,
            &registry
        ));

        let mut full_inventory = Player::new("char_3", "Tester", 0, 0, "m", "skin1", None, None);
        full_inventory.inventory.slots[0] =
            Some(crate::item::InventorySlot::new("ore".to_string(), 2));
        for i in 1..crate::item::INVENTORY_SIZE {
            full_inventory.inventory.slots[i] =
                Some(crate::item::InventorySlot::new(format!("item_{}", i), 99));
        }
        assert!(!can_continue_batch(&full_inventory, &recipe, &registry));
    }
}
