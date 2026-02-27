use super::*;
use crate::protocol::{DialogueChoice, FarmingPatchData, TileOverride};

const MASTER_FARMER_NAME: &str = "Master Farmer";
const LOCKED_PLOT_TILE_ID: u32 = 65;
const UNLOCKED_PLOT_TILE_ID: u32 = 62;

fn inventory_update_message(player_id: &str, inventory: &Inventory) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        player_id: player_id.to_string(),
        slots: inventory.to_update(),
        gold: inventory.gold,
    }
}

fn farming_xp_message(player_id: &str, xp_gained: i64, total_xp: i64, level: i32) -> ServerMessage {
    ServerMessage::SkillXp {
        player_id: player_id.to_string(),
        skill: "farming".to_string(),
        xp_gained,
        total_xp,
        level,
    }
}

fn patch_state_update(
    patch_id: &str,
    state: &str,
    crop_id: &str,
    growth_stage: u32,
    owner_id: &str,
) -> ServerMessage {
    ServerMessage::PatchStateUpdate {
        patch_id: patch_id.to_string(),
        state: state.to_string(),
        crop_id: crop_id.to_string(),
        growth_stage,
        owner_id: owner_id.to_string(),
    }
}

fn plot_purchase_choice(
    req: &crate::farming::PlotRequirement,
    owned: bool,
    farming_level: i32,
    gold: i32,
) -> DialogueChoice {
    if owned {
        DialogueChoice {
            id: format!("owned_{}", req.plot_id),
            text: format!("Plot {} (Owned)", req.plot_id),
        }
    } else if farming_level < req.farming_level {
        DialogueChoice {
            id: format!("locked_{}", req.plot_id),
            text: format!(
                "Plot {} - {}gp (Requires Farming {})",
                req.plot_id, req.gold_cost, req.farming_level
            ),
        }
    } else if gold < req.gold_cost {
        DialogueChoice {
            id: format!("locked_{}", req.plot_id),
            text: format!(
                "Plot {} - {}gp (Not enough gold)",
                req.plot_id, req.gold_cost
            ),
        }
    } else {
        DialogueChoice {
            id: format!("unlock_{}", req.plot_id),
            text: format!("Plot {} - {}gp", req.plot_id, req.gold_cost),
        }
    }
}

fn contract_choice(
    difficulty: &crate::farming::ContractDifficulty,
    farming_level: i32,
) -> DialogueChoice {
    if farming_level >= difficulty.level_required() {
        DialogueChoice {
            id: format!("accept_{}", difficulty.as_str()),
            text: format!(
                "{} - {}xp, {}gp",
                difficulty.display_name(),
                difficulty.xp_reward(),
                difficulty.gold_reward()
            ),
        }
    } else {
        DialogueChoice {
            id: format!("locked_{}", difficulty.as_str()),
            text: format!(
                "{} (Requires Farming {})",
                difficulty.display_name(),
                difficulty.level_required()
            ),
        }
    }
}

fn plot_tile_id(is_unlocked: bool) -> u32 {
    if is_unlocked {
        UNLOCKED_PLOT_TILE_ID
    } else {
        LOCKED_PLOT_TILE_ID
    }
}

fn farming_contract_message(
    contract: Option<(&crate::farming::FarmingContract, String)>,
) -> ServerMessage {
    match contract {
        Some((contract, crop_name)) => ServerMessage::FarmingContractUpdate {
            active: true,
            difficulty: contract.difficulty.display_name().to_string(),
            crop_name,
            amount_required: contract.amount_required,
            amount_harvested: contract.amount_harvested,
        },
        None => ServerMessage::FarmingContractUpdate {
            active: false,
            difficulty: String::new(),
            crop_name: String::new(),
            amount_required: 0,
            amount_harvested: 0,
        },
    }
}

impl GameRoom {
    async fn master_farmer_name(&self, npc_id: &str) -> String {
        let npcs = self.npcs.read().await;
        npcs.get(npc_id)
            .and_then(|npc| self.entity_registry.get(&npc.prototype_id))
            .map(|proto| proto.display_name.clone())
            .unwrap_or_else(|| MASTER_FARMER_NAME.to_string())
    }

    pub async fn handle_plant_seed(&self, player_id: &str, patch_id: &str, item_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let farming_level = {
            let players = self.players.read().await;
            let Some(player) = players.get(player_id) else {
                return;
            };
            if player.inventory.has_item(item_id, 1) {
                Some(player.skills.farming.level)
            } else {
                None
            }
        };

        let Some(farming_level) = farming_level else {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You don't have that seed".to_string(),
                },
            )
            .await;
            return;
        };

        let result = {
            let mut farming = self.farming.write().await;
            farming.plant_seed(patch_id, item_id, player_id, farming_level, current_time)
        };

        match result {
            Ok((crop_id, xp)) => {
                let (inv_msg, total_xp, level, leveled_up) = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };
                    player.inventory.remove_item(item_id, 1);
                    let leveled_up = player.skills.farming.add_xp(xp);
                    let total_xp = player.skills.farming.xp;
                    let level = player.skills.farming.level;
                    (
                        inventory_update_message(player_id, &player.inventory),
                        total_xp,
                        level,
                        leveled_up,
                    )
                };

                self.send_to_player(player_id, inv_msg).await;
                self.send_to_player(
                    player_id,
                    farming_xp_message(player_id, xp, total_xp, level),
                )
                .await;

                if leveled_up {
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: level,
                    })
                    .await;
                    self.process_quest_progression_snapshot(player_id).await;
                }

                if let Some(ref db) = self.db {
                    if let Err(e) = db
                        .save_farming_patch(patch_id, player_id, &crop_id, current_time)
                        .await
                    {
                        tracing::warn!("Failed to save farming patch {}: {}", patch_id, e);
                    }
                }

                self.send_to_player(
                    player_id,
                    patch_state_update(patch_id, "growing", &crop_id, 0, player_id),
                )
                .await;

                self.process_quest_item_collect(player_id, &format!("plant_{}", item_id), 1)
                    .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_harvest_crop(&self, player_id: &str, patch_id: &str) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let farming_level = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|player| player.skills.farming.level)
                .unwrap_or(1)
        };

        let inventory_full = {
            let farming = self.farming.read().await;
            let players = self.players.read().await;
            let key = (patch_id.to_string(), player_id.to_string());
            farming
                .player_states
                .get(&key)
                .and_then(|state| farming.crops.get(&state.crop_id))
                .map(|crop| {
                    players.get(player_id).is_some_and(|player| {
                        !player.inventory.has_space_for(
                            &crop.produce_item,
                            crop.harvest_amount_max,
                            &self.item_registry,
                        )
                    })
                })
                .unwrap_or(false)
        };

        if inventory_full {
            self.send_system_message(player_id, "Your inventory is full.")
                .await;
            return;
        }

        let result = {
            let mut farming = self.farming.write().await;
            farming.harvest_crop(patch_id, player_id, current_time, farming_level)
        };

        match result {
            Ok(harvest) => {
                let (inv_msg, total_xp, level, leveled_up) = {
                    let mut players = self.players.write().await;
                    let Some(player) = players.get_mut(player_id) else {
                        return;
                    };

                    player.inventory.add_item(
                        &harvest.produce_item,
                        harvest.amount,
                        &self.item_registry,
                    );
                    if harvest.seed_returned {
                        player
                            .inventory
                            .add_item(&harvest.seed_item, 1, &self.item_registry);
                    }

                    let leveled_up = player.skills.farming.add_xp(harvest.xp_gained);
                    let total_xp = player.skills.farming.xp;
                    let level = player.skills.farming.level;

                    (
                        inventory_update_message(player_id, &player.inventory),
                        total_xp,
                        level,
                        leveled_up,
                    )
                };

                self.send_to_player(player_id, inv_msg).await;
                self.send_to_player(
                    player_id,
                    farming_xp_message(player_id, harvest.xp_gained, total_xp, level),
                )
                .await;

                if leveled_up {
                    self.broadcast(ServerMessage::SkillLevelUp {
                        player_id: player_id.to_string(),
                        skill: "farming".to_string(),
                        new_level: level,
                    })
                    .await;
                    self.process_quest_progression_snapshot(player_id).await;
                }

                self.send_to_player(player_id, patch_state_update(patch_id, "empty", "", 0, ""))
                    .await;

                if let Some(ref db) = self.db {
                    if let Err(e) = db.delete_farming_patch(patch_id, player_id).await {
                        tracing::warn!("Failed to delete farming patch {}: {}", patch_id, e);
                    }
                }

                self.process_quest_item_collect(
                    player_id,
                    &format!("harvest_{}", harvest.produce_item),
                    harvest.amount,
                )
                .await;
                self.process_quest_item_collect(player_id, &harvest.produce_item, harvest.amount)
                    .await;

                let contract_progress = {
                    let mut farming = self.farming.write().await;
                    let crop_id = farming
                        .crops
                        .iter()
                        .find(|(_, crop)| crop.produce_item == harvest.produce_item)
                        .map(|(crop_id, _)| crop_id.clone());

                    crop_id.and_then(|crop_id| {
                        farming.record_contract_harvest(player_id, &crop_id, harvest.amount)
                    })
                };

                if let Some((harvested, required, complete)) = contract_progress {
                    if complete {
                        self.send_system_message(
                            player_id,
                            &format!(
                                "Contract complete! ({}/{}) Return to the Master Farmer to claim your rewards.",
                                harvested, required
                            ),
                        )
                        .await;
                    } else {
                        self.send_system_message(
                            player_id,
                            &format!("Contract progress: {}/{} harvested.", harvested, required),
                        )
                        .await;
                    }

                    if let Some(ref db) = self.db {
                        if let Err(e) = db
                            .update_farming_contract_progress(player_id, harvested)
                            .await
                        {
                            tracing::warn!("Failed to update contract progress: {}", e);
                        }
                    }

                    self.send_farming_contract_update(player_id).await;
                }
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub(super) async fn show_plot_purchase_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = self.master_farmer_name(npc_id).await;
        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.skills.farming.level, player.inventory.gold),
                None => return,
            }
        };
        let unlocked = {
            let farming = self.farming.read().await;
            farming.get_unlocked_plots(player_id)
        };

        let mut choices: Vec<DialogueChoice> = crate::farming::PLOT_REQUIREMENTS
            .iter()
            .map(|req| {
                plot_purchase_choice(req, unlocked.contains(&req.plot_id), farming_level, gold)
            })
            .collect();
        choices.push(DialogueChoice {
            id: "nevermind".to_string(),
            text: "Go back".to_string(),
        });

        let text = format!(
            "Each allotment plot gives you 16 farming patches to grow crops.\n\nYour gold: {}gp | Farming level: {}",
            gold, farming_level
        );

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("plot_seller:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text,
                choices,
            },
        )
        .await;
    }

    pub(super) async fn show_master_farmer_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = self.master_farmer_name(npc_id).await;
        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("plot_seller:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text: "Ah, welcome! I've been tending these fields for decades. What can I help you with?".to_string(),
                choices: vec![
                    DialogueChoice {
                        id: "contracts".to_string(),
                        text: "Farming contracts".to_string(),
                    },
                    DialogueChoice {
                        id: "buy_plots".to_string(),
                        text: "Buy allotment plot".to_string(),
                    },
                    DialogueChoice {
                        id: "close".to_string(),
                        text: "Nevermind".to_string(),
                    },
                ],
            },
        )
        .await;
    }

    pub(super) async fn show_contract_dialogue(&self, player_id: &str, npc_id: &str) {
        let npc_name = self.master_farmer_name(npc_id).await;
        let farming_level = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => player.skills.farming.level,
                None => return,
            }
        };

        let active_contract = {
            let farming = self.farming.read().await;
            farming.get_contract(player_id).cloned().map(|contract| {
                let crop_name = farming
                    .crops
                    .get(&contract.crop_id)
                    .map(|crop| crop.produce_item.clone())
                    .unwrap_or_else(|| contract.crop_id.clone());
                (contract, crop_name)
            })
        };

        if let Some((contract, crop_name)) = active_contract {
            let (text, choices) = if contract.is_complete() {
                (
                    format!(
                        "Well done! You've completed your {} contract. Harvested: {}/{} {}. Rewards: {} Farming XP, {}gp, and {} bonus seed(s).",
                        contract.difficulty.display_name(),
                        contract.amount_harvested,
                        contract.amount_required,
                        crop_name,
                        contract.difficulty.xp_reward(),
                        contract.difficulty.gold_reward(),
                        contract.difficulty.seed_reward_count(),
                    ),
                    vec![
                        DialogueChoice {
                            id: "claim_contract".to_string(),
                            text: "Claim rewards".to_string(),
                        },
                        DialogueChoice {
                            id: "nevermind".to_string(),
                            text: "Go back".to_string(),
                        },
                    ],
                )
            } else {
                (
                    format!(
                        "You have an active {} contract: Harvest {} {} ({}/{}). Keep at it!",
                        contract.difficulty.display_name(),
                        contract.amount_required,
                        crop_name,
                        contract.amount_harvested,
                        contract.amount_required,
                    ),
                    vec![
                        DialogueChoice {
                            id: "abandon_contract".to_string(),
                            text: "Abandon contract".to_string(),
                        },
                        DialogueChoice {
                            id: "nevermind".to_string(),
                            text: "Go back".to_string(),
                        },
                    ],
                )
            };

            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: format!("plot_seller:{}", npc_id),
                    npc_id: npc_id.to_string(),
                    speaker: npc_name,
                    text,
                    choices,
                },
            )
            .await;
            return;
        }

        let mut choices = vec![
            contract_choice(&crate::farming::ContractDifficulty::Easy, farming_level),
            contract_choice(&crate::farming::ContractDifficulty::Medium, farming_level),
            contract_choice(&crate::farming::ContractDifficulty::Hard, farming_level),
        ];
        choices.push(DialogueChoice {
            id: "nevermind".to_string(),
            text: "Go back".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("plot_seller:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: npc_name,
                text: "I've got work that needs doing. Pick a contract and harvest the crops I need. You can have one active contract at a time.".to_string(),
                choices,
            },
        )
        .await;
    }

    pub(super) async fn handle_accept_contract(&self, player_id: &str, difficulty_str: &str) {
        let Some(difficulty) = crate::farming::ContractDifficulty::from_str(difficulty_str) else {
            self.send_system_message(player_id, "Invalid contract difficulty.")
                .await;
            return;
        };

        let farming_level = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => player.skills.farming.level,
                None => return,
            }
        };

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let contract_result = {
            let mut farming = self.farming.write().await;
            match farming.generate_contract(player_id, &difficulty, farming_level, current_time) {
                Ok(contract) => {
                    let crop_id = contract.crop_id.clone();
                    let amount = contract.amount_required;
                    let crop_name = farming
                        .crops
                        .get(&crop_id)
                        .map(|crop| crop.produce_item.clone())
                        .unwrap_or(crop_id);
                    Ok((crop_name, amount))
                }
                Err(message) => Err(message),
            }
        };

        let (crop_name, amount) = match contract_result {
            Ok(result) => result,
            Err(message) => {
                self.send_system_message(player_id, &message).await;
                return;
            }
        };

        if let Some(ref db) = self.db {
            let contract = {
                let farming = self.farming.read().await;
                farming.get_contract(player_id).map(|contract| {
                    (
                        contract.difficulty.as_str().to_string(),
                        contract.crop_id.clone(),
                        contract.amount_required,
                        contract.amount_harvested,
                        contract.created_at,
                    )
                })
            };

            if let Some((difficulty, crop_id, amount_required, amount_harvested, created_at)) =
                contract
            {
                if let Err(e) = db
                    .save_farming_contract(
                        player_id,
                        &difficulty,
                        &crop_id,
                        amount_required,
                        amount_harvested,
                        created_at,
                    )
                    .await
                {
                    tracing::error!("Failed to save farming contract: {}", e);
                }
            }
        }

        self.send_system_message(
            player_id,
            &format!("Contract accepted! Harvest {} {}.", amount, crop_name),
        )
        .await;
        self.send_farming_contract_update(player_id).await;
    }

    pub(super) async fn handle_claim_contract(&self, player_id: &str) {
        let contract_info = {
            let farming = self.farming.read().await;
            match farming.get_contract(player_id) {
                Some(contract) if contract.is_complete() => Some((
                    contract.difficulty.xp_reward(),
                    contract.difficulty.gold_reward(),
                    contract.difficulty.seed_reward_count(),
                    contract.difficulty.display_name().to_string(),
                )),
                Some(_) => {
                    self.send_system_message(player_id, "Your contract isn't complete yet.")
                        .await;
                    return;
                }
                None => {
                    self.send_system_message(player_id, "You don't have an active contract.")
                        .await;
                    return;
                }
            }
        };

        let Some((xp_reward, gold_reward, seed_count, diff_name)) = contract_info else {
            self.send_system_message(player_id, "Your contract isn't complete yet.")
                .await;
            return;
        };

        let (inv_msg, skill_xp, skill_level, leveled_up, seed_names) = {
            let farming = self.farming.read().await;
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };

            player.inventory.gold += gold_reward;

            let mut seed_names = Vec::new();
            for _ in 0..seed_count {
                if let Some(seed_id) = farming.random_seed_for_level(player.skills.farming.level) {
                    let seed_name = self
                        .item_registry
                        .get(&seed_id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| seed_id.clone());
                    player.inventory.add_item(&seed_id, 1, &self.item_registry);
                    seed_names.push(seed_name);
                }
            }

            let leveled_up = player.skills.farming.add_xp(xp_reward);
            let skill_xp = player.skills.farming.xp;
            let skill_level = player.skills.farming.level;

            (
                inventory_update_message(player_id, &player.inventory),
                skill_xp,
                skill_level,
                leveled_up,
                seed_names,
            )
        };

        {
            let mut farming = self.farming.write().await;
            if farming.remove_contract(player_id).is_none() {
                self.send_system_message(player_id, "You don't have an active contract.")
                    .await;
                return;
            }
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db.delete_farming_contract(player_id).await {
                tracing::error!("Failed to delete farming contract: {}", e);
            }
        }

        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            farming_xp_message(player_id, xp_reward, skill_xp, skill_level),
        )
        .await;

        if leveled_up {
            self.broadcast(ServerMessage::SkillLevelUp {
                player_id: player_id.to_string(),
                skill: "farming".to_string(),
                new_level: skill_level,
            })
            .await;
            self.process_quest_progression_snapshot(player_id).await;
        }

        let seed_text = if seed_names.is_empty() {
            String::new()
        } else {
            format!(" and {}", seed_names.join(", "))
        };

        self.send_system_message(
            player_id,
            &format!(
                "{} contract complete! Received {} Farming XP, {}gp{}.",
                diff_name, xp_reward, gold_reward, seed_text
            ),
        )
        .await;
        self.send_farming_contract_update(player_id).await;
    }

    pub(super) async fn handle_abandon_contract(&self, player_id: &str) {
        let removed_contract = {
            let mut farming = self.farming.write().await;
            farming.remove_contract(player_id)
        };

        if removed_contract.is_none() {
            self.send_system_message(player_id, "You don't have an active contract.")
                .await;
            return;
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db.delete_farming_contract(player_id).await {
                tracing::error!("Failed to delete farming contract: {}", e);
            }
        }

        self.send_system_message(player_id, "Contract abandoned.")
            .await;
        self.send_farming_contract_update(player_id).await;
    }

    pub(super) async fn handle_plot_purchase(&self, player_id: &str, plot_id: u32) {
        let Some(req) = crate::farming::PLOT_REQUIREMENTS
            .iter()
            .find(|req| req.plot_id == plot_id)
        else {
            self.send_system_message(player_id, "Invalid plot.").await;
            return;
        };

        let already_unlocked = {
            let farming = self.farming.read().await;
            farming.is_plot_unlocked(player_id, plot_id)
        };
        if already_unlocked {
            self.send_system_message(player_id, "You already own this plot.")
                .await;
            return;
        }

        let (farming_level, gold) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.skills.farming.level, player.inventory.gold),
                None => return,
            }
        };

        if farming_level < req.farming_level {
            self.send_system_message(
                player_id,
                &format!(
                    "You need Farming level {} to unlock this plot.",
                    req.farming_level
                ),
            )
            .await;
            return;
        }

        if gold < req.gold_cost {
            self.send_system_message(
                player_id,
                &format!(
                    "You need {}gp to unlock this plot. You have {}gp.",
                    req.gold_cost, gold
                ),
            )
            .await;
            return;
        }

        let inv_msg = {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };
            player.inventory.gold -= req.gold_cost;
            inventory_update_message(player_id, &player.inventory)
        };

        self.send_to_player(player_id, inv_msg).await;

        {
            let mut farming = self.farming.write().await;
            farming.unlock_plot(player_id, plot_id);
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db.save_plot_unlock(player_id, plot_id).await {
                tracing::error!("Failed to save plot unlock: {}", e);
            }
        }

        self.send_system_message(
            player_id,
            &format!(
                "You've unlocked Plot {}! 16 new allotment patches are now available.",
                plot_id
            ),
        )
        .await;

        let patches_msg = self.get_farming_patches_message(player_id).await;
        self.send_to_player(player_id, patches_msg).await;
    }

    pub async fn get_farming_patches_message(&self, player_id: &str) -> ServerMessage {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let farming = self.farming.read().await;
        let updates = farming.get_player_patch_states(player_id, current_time);
        let patches: Vec<FarmingPatchData> = updates
            .into_iter()
            .map(|update| {
                let patch = farming.patches.get(&update.patch_id).unwrap();
                FarmingPatchData {
                    patch_id: update.patch_id,
                    x: patch.x,
                    y: patch.y,
                    state: update.state,
                    crop_id: update.crop_id,
                    growth_stage: update.growth_stage,
                    owner_id: update.owner_id,
                }
            })
            .collect();
        let unlocked_plots = farming.get_unlocked_plots(player_id);
        let tile_overrides: Vec<TileOverride> = farming
            .patches
            .values()
            .map(|patch| TileOverride {
                x: patch.x,
                y: patch.y,
                tile_id: plot_tile_id(farming.is_plot_unlocked(player_id, patch.plot)),
            })
            .collect();

        ServerMessage::FarmingPatchStates {
            patches,
            unlocked_plots,
            tile_overrides,
        }
    }

    pub async fn get_farming_contract_message(&self, player_id: &str) -> ServerMessage {
        let farming = self.farming.read().await;
        let contract = farming.get_contract(player_id).map(|contract| {
            let crop_name = farming
                .crops
                .get(&contract.crop_id)
                .map(|crop| crop.produce_item.clone())
                .unwrap_or_else(|| contract.crop_id.clone());
            (contract, crop_name)
        });
        farming_contract_message(contract)
    }

    pub async fn send_farming_contract_update(&self, player_id: &str) {
        let msg = self.get_farming_contract_message(player_id).await;
        self.send_to_player(player_id, msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_purchase_choice_reflects_owned_locked_and_affordable_states() {
        let req = crate::farming::PlotRequirement {
            plot_id: 2,
            farming_level: 15,
            gold_cost: 500,
        };

        let owned = plot_purchase_choice(&req, true, 99, 9999);
        assert_eq!(owned.id, "owned_2");
        assert_eq!(owned.text, "Plot 2 (Owned)");

        let level_locked = plot_purchase_choice(&req, false, 10, 9999);
        assert_eq!(level_locked.id, "locked_2");
        assert!(level_locked.text.contains("Requires Farming 15"));

        let gold_locked = plot_purchase_choice(&req, false, 20, 100);
        assert_eq!(gold_locked.id, "locked_2");
        assert!(gold_locked.text.contains("Not enough gold"));

        let affordable = plot_purchase_choice(&req, false, 20, 500);
        assert_eq!(affordable.id, "unlock_2");
        assert_eq!(affordable.text, "Plot 2 - 500gp");
    }

    #[test]
    fn contract_choice_uses_accept_or_locked_variants_by_level() {
        let easy = contract_choice(&crate::farming::ContractDifficulty::Easy, 1);
        assert_eq!(easy.id, "accept_easy");
        assert!(easy.text.contains("150xp"));

        let hard = contract_choice(&crate::farming::ContractDifficulty::Hard, 10);
        assert_eq!(hard.id, "locked_hard");
        assert!(hard.text.contains("Requires Farming 30"));
    }

    #[test]
    fn farming_contract_message_reports_active_and_empty_states() {
        let contract = crate::farming::FarmingContract {
            player_id: "p1".to_string(),
            difficulty: crate::farming::ContractDifficulty::Medium,
            crop_id: "cabbage".to_string(),
            amount_required: 8,
            amount_harvested: 3,
            created_at: 123,
        };

        match farming_contract_message(Some((&contract, "cabbage".to_string()))) {
            ServerMessage::FarmingContractUpdate {
                active,
                difficulty,
                crop_name,
                amount_required,
                amount_harvested,
            } => {
                assert!(active);
                assert_eq!(difficulty, "Medium");
                assert_eq!(crop_name, "cabbage");
                assert_eq!(amount_required, 8);
                assert_eq!(amount_harvested, 3);
            }
            other => panic!("unexpected message: {:?}", other.msg_type()),
        }

        match farming_contract_message(None) {
            ServerMessage::FarmingContractUpdate {
                active,
                difficulty,
                crop_name,
                amount_required,
                amount_harvested,
            } => {
                assert!(!active);
                assert!(difficulty.is_empty());
                assert!(crop_name.is_empty());
                assert_eq!(amount_required, 0);
                assert_eq!(amount_harvested, 0);
            }
            other => panic!("unexpected message: {:?}", other.msg_type()),
        }
    }

    #[test]
    fn plot_tile_id_matches_locked_and_unlocked_tiles() {
        assert_eq!(plot_tile_id(true), UNLOCKED_PLOT_TILE_ID);
        assert_eq!(plot_tile_id(false), LOCKED_PLOT_TILE_ID);
    }
}
