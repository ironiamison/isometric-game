use super::GameRoom;
use crate::protocol::{
    AdventureBoardActiveContractData, AdventureBoardDifficultyData, AdventureBoardOfferData,
    AdventureBoardStatsData, DialogueChoice, ServerMessage,
};
use crate::resource_contracts::{ContractDifficulty, ResourceContract, ResourceContractKind};
use rand::Rng;
use std::collections::HashMap;

const ADVENTURE_BOARD_NAME: &str = "Adventure Board";

enum ClaimContractStatus {
    Ready {
        contract: ResourceContract,
        xp_reward: i64,
        gold_reward: i32,
        seed_count: i32,
    },
    Incomplete,
    Missing,
}

struct ContractTarget {
    item_id: String,
    target_name: String,
    level_required: i32,
}

fn resource_contract_choice(
    difficulty: ContractDifficulty,
    player_level: i32,
    kind: ResourceContractKind,
) -> DialogueChoice {
    if player_level >= difficulty.level_required() {
        DialogueChoice {
            id: format!("accept_{}", difficulty.as_str()),
            text: format!(
                "{} - {} {} XP, {}gp",
                difficulty.display_name(),
                difficulty.xp_reward(kind),
                kind.display_name(),
                difficulty.gold_reward(kind)
            ),
        }
    } else {
        DialogueChoice {
            id: format!("locked_{}", difficulty.as_str()),
            text: format!(
                "{} (Requires {} {})",
                difficulty.display_name(),
                kind.display_name(),
                difficulty.level_required()
            ),
        }
    }
}

fn resource_contract_message(contract: Option<&ResourceContract>) -> ServerMessage {
    match contract {
        Some(contract) => ServerMessage::ResourceContractUpdate {
            active: true,
            contract_kind: contract.kind.display_name().to_string(),
            difficulty: contract.difficulty.display_name().to_string(),
            task_text: contract.task_text(),
            progress_label: contract.kind.progress_label().to_string(),
            amount_required: contract.amount_required,
            amount_completed: contract.amount_completed,
            giver_name: contract.giver_name.clone(),
        },
        None => ServerMessage::ResourceContractUpdate {
            active: false,
            contract_kind: String::new(),
            difficulty: String::new(),
            task_text: String::new(),
            progress_label: String::new(),
            amount_required: 0,
            amount_completed: 0,
            giver_name: String::new(),
        },
    }
}

fn kind_for_entity_type(entity_type: &str) -> Option<ResourceContractKind> {
    match entity_type {
        "master_farmer" => Some(ResourceContractKind::Farming),
        "miner_mike" => Some(ResourceContractKind::Mining),
        "lumberjack_pete" => Some(ResourceContractKind::Woodcutting),
        _ => None,
    }
}

fn required_quest_for_contract_npc(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "miner_mike" => Some("rock_bottom"),
        "lumberjack_pete" => Some("axe_to_grind"),
        _ => None,
    }
}

fn adventure_board_description(kind: ResourceContractKind) -> &'static str {
    match kind {
        ResourceContractKind::Farming => {
            "Short field orders for growers with fresh produce on hand."
        }
        ResourceContractKind::Mining => "Ore runs and extraction jobs for the village stores.",
        ResourceContractKind::Woodcutting => "Log requests for builders, fires, and tool handles.",
        ResourceContractKind::Fishing => "Daily fish hauls for cooks, traders, and camp stock.",
        ResourceContractKind::Smithing => "Forge commissions that turn raw stock into useful gear.",
    }
}

fn adventure_board_kinds() -> [ResourceContractKind; 5] {
    [
        ResourceContractKind::Farming,
        ResourceContractKind::Mining,
        ResourceContractKind::Woodcutting,
        ResourceContractKind::Fishing,
        ResourceContractKind::Smithing,
    ]
}

impl GameRoom {
    pub(in crate::game) fn resource_contract_kind_for_entity(
        &self,
        entity_type: &str,
    ) -> Option<ResourceContractKind> {
        kind_for_entity_type(entity_type)
    }

    pub(in crate::game) async fn resource_contract_npc_unlocked(
        &self,
        player_id: &str,
        entity_type: &str,
    ) -> bool {
        let Some(required_quest) = required_quest_for_contract_npc(entity_type) else {
            return true;
        };

        let quest_states = self.player_quest_states.read().await;
        quest_states
            .get(player_id)
            .map(|state| state.is_quest_completed(required_quest))
            .unwrap_or(false)
    }

    pub(in crate::game) async fn show_resource_contract_master_dialogue(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
    ) {
        let Some(prototype) = self.entity_registry.get(entity_type) else {
            return;
        };

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("resource_contract_master:{}:{}", npc_id, entity_type),
                npc_id: npc_id.to_string(),
                speaker: prototype.display_name.clone(),
                text: prototype.dialogue.greeting.clone().unwrap_or_else(|| {
                    format!(
                        "Need {} work or want to browse the shop?",
                        prototype.display_name
                    )
                }),
                choices: vec![
                    DialogueChoice {
                        id: "contracts".to_string(),
                        text: "Show contracts".to_string(),
                    },
                    DialogueChoice {
                        id: "open_shop".to_string(),
                        text: "Open shop".to_string(),
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

    async fn adventure_board_message(&self, player_id: &str, npc_id: &str) -> ServerMessage {
        let active_contract = {
            let resource_contracts = self.resource_contracts.read().await;
            resource_contracts.get_contract(player_id).cloned()
        };

        let mut offers = Vec::new();
        for kind in adventure_board_kinds() {
            let Some(skill_level) = self.player_skill_level_for_contract(player_id, kind).await
            else {
                continue;
            };

            let difficulties = [
                ContractDifficulty::Easy,
                ContractDifficulty::Medium,
                ContractDifficulty::Hard,
            ]
            .into_iter()
            .map(|difficulty| AdventureBoardDifficultyData {
                difficulty_id: difficulty.as_str().to_string(),
                difficulty_name: difficulty.display_name().to_string(),
                level_required: difficulty.level_required(),
                unlocked: skill_level >= difficulty.level_required(),
                reward_xp: difficulty.xp_reward(kind),
                reward_gold: difficulty.gold_reward(kind),
            })
            .collect();

            offers.push(AdventureBoardOfferData {
                kind_id: kind.as_str().to_string(),
                kind_name: kind.display_name().to_string(),
                description: adventure_board_description(kind).to_string(),
                skill_level,
                difficulties,
            });
        }

        let active_contract = active_contract.map(|contract| AdventureBoardActiveContractData {
            kind_id: contract.kind.as_str().to_string(),
            kind_name: contract.kind.display_name().to_string(),
            difficulty_name: contract.difficulty.display_name().to_string(),
            task_text: contract.task_text(),
            progress_label: contract.kind.progress_label().to_string(),
            amount_required: contract.amount_required,
            amount_completed: contract.amount_completed,
            giver_name: contract.giver_name.clone(),
            reward_xp: contract.difficulty.xp_reward(contract.kind),
            reward_gold: contract.difficulty.gold_reward(contract.kind),
            bonus_item_text: if contract.kind == ResourceContractKind::Farming {
                format!(
                    "{} bonus seed(s)",
                    contract.difficulty.farming_seed_reward_count()
                )
            } else {
                String::new()
            },
            can_claim: contract.is_complete(),
        });

        let stats = if let Some(ref db) = self.db {
            match db.get_resource_contract_stats(player_id).await {
                Ok((contracts_completed, total_gold_earned, total_xp_earned)) => {
                    AdventureBoardStatsData {
                        contracts_completed,
                        total_gold_earned,
                        total_xp_earned,
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load adventure board stats: {}", e);
                    AdventureBoardStatsData {
                        contracts_completed: 0,
                        total_gold_earned: 0,
                        total_xp_earned: 0,
                    }
                }
            }
        } else {
            AdventureBoardStatsData {
                contracts_completed: 0,
                total_gold_earned: 0,
                total_xp_earned: 0,
            }
        };

        ServerMessage::AdventureBoardState {
            npc_id: npc_id.to_string(),
            offers,
            active_contract,
            stats,
        }
    }

    pub(in crate::game) async fn show_adventure_board_dialogue(
        &self,
        player_id: &str,
        npc_id: &str,
    ) {
        let msg = self.adventure_board_message(player_id, npc_id).await;
        self.send_to_player(player_id, msg).await;
    }

    pub(in crate::game) async fn show_resource_contract_dialogue(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
    ) {
        let Some(kind) = kind_for_entity_type(entity_type) else {
            return;
        };
        let Some(prototype) = self.entity_registry.get(entity_type) else {
            return;
        };
        let Some(player_level) = self.player_skill_level_for_contract(player_id, kind).await else {
            return;
        };

        let active_contract = {
            let resource_contracts = self.resource_contracts.read().await;
            resource_contracts.get_contract(player_id).cloned()
        };

        if let Some(contract) = active_contract {
            let (text, choices) = if contract.is_complete() {
                let reward_text = if contract.kind == ResourceContractKind::Farming {
                    format!(
                        "{} {} XP, {}gp, and {} bonus seed(s)",
                        contract.difficulty.xp_reward(contract.kind),
                        contract.kind.display_name(),
                        contract.difficulty.gold_reward(contract.kind),
                        contract.difficulty.farming_seed_reward_count()
                    )
                } else {
                    format!(
                        "{} {} XP and {}gp",
                        contract.difficulty.xp_reward(contract.kind),
                        contract.kind.display_name(),
                        contract.difficulty.gold_reward(contract.kind)
                    )
                };
                (
                    format!(
                        "Your {} contract from {} is complete: {} ({}/{} {}). Rewards: {}.",
                        contract.kind.display_name(),
                        contract.giver_name,
                        contract.task_text(),
                        contract.amount_completed,
                        contract.amount_required,
                        contract.kind.progress_label(),
                        reward_text
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
                        "You already have an active {} contract from {}: {} ({}/{} {}). You can only carry one resource contract at a time.",
                        contract.kind.display_name(),
                        contract.giver_name,
                        contract.task_text(),
                        contract.amount_completed,
                        contract.amount_required,
                        contract.kind.progress_label()
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
                    quest_id: format!("resource_contract_master:{}:{}", npc_id, entity_type),
                    npc_id: npc_id.to_string(),
                    speaker: prototype.display_name.clone(),
                    text,
                    choices,
                },
            )
            .await;
            return;
        }

        let mut choices = vec![
            resource_contract_choice(ContractDifficulty::Easy, player_level, kind),
            resource_contract_choice(ContractDifficulty::Medium, player_level, kind),
            resource_contract_choice(ContractDifficulty::Hard, player_level, kind),
        ];
        choices.push(DialogueChoice {
            id: "nevermind".to_string(),
            text: "Go back".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("resource_contract_master:{}:{}", npc_id, entity_type),
                npc_id: npc_id.to_string(),
                speaker: prototype.display_name.clone(),
                text: format!(
                    "I've got {} work that needs doing. Pick a contract and bring back the materials I ask for. You can only carry one resource contract at a time.",
                    kind.skill_name()
                ),
                choices,
            },
        )
        .await;
    }

    pub(in crate::game) async fn show_adventure_board_contract_dialogue(
        &self,
        player_id: &str,
        npc_id: &str,
        kind: ResourceContractKind,
    ) {
        let Some(player_level) = self.player_skill_level_for_contract(player_id, kind).await else {
            return;
        };

        let active_contract = {
            let resource_contracts = self.resource_contracts.read().await;
            resource_contracts.get_contract(player_id).cloned()
        };

        if let Some(contract) = active_contract {
            let (text, choices) = if contract.is_complete() {
                let reward_text = if contract.kind == ResourceContractKind::Farming {
                    format!(
                        "{} {} XP, {}gp, and {} bonus seed(s)",
                        contract.difficulty.xp_reward(contract.kind),
                        contract.kind.display_name(),
                        contract.difficulty.gold_reward(contract.kind),
                        contract.difficulty.farming_seed_reward_count()
                    )
                } else {
                    format!(
                        "{} {} XP and {}gp",
                        contract.difficulty.xp_reward(contract.kind),
                        contract.kind.display_name(),
                        contract.difficulty.gold_reward(contract.kind)
                    )
                };
                (
                    format!(
                        "Your {} contract is complete: {} ({}/{} {}). Rewards: {}.",
                        contract.kind.display_name(),
                        contract.task_text(),
                        contract.amount_completed,
                        contract.amount_required,
                        contract.kind.progress_label(),
                        reward_text
                    ),
                    vec![
                        DialogueChoice {
                            id: "claim_contract".to_string(),
                            text: "Claim rewards".to_string(),
                        },
                        DialogueChoice {
                            id: "nevermind".to_string(),
                            text: "Back to board".to_string(),
                        },
                    ],
                )
            } else {
                (
                    format!(
                        "You already have an active {} contract: {} ({}/{} {}). You can only carry one resource contract at a time.",
                        contract.kind.display_name(),
                        contract.task_text(),
                        contract.amount_completed,
                        contract.amount_required,
                        contract.kind.progress_label()
                    ),
                    vec![
                        DialogueChoice {
                            id: "abandon_contract".to_string(),
                            text: "Abandon contract".to_string(),
                        },
                        DialogueChoice {
                            id: "nevermind".to_string(),
                            text: "Back to board".to_string(),
                        },
                    ],
                )
            };

            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: format!("adventure_board_contract:{}:{}", npc_id, kind.as_str()),
                    npc_id: npc_id.to_string(),
                    speaker: ADVENTURE_BOARD_NAME.to_string(),
                    text,
                    choices,
                },
            )
            .await;
            return;
        }

        let mut choices = vec![
            resource_contract_choice(ContractDifficulty::Easy, player_level, kind),
            resource_contract_choice(ContractDifficulty::Medium, player_level, kind),
            resource_contract_choice(ContractDifficulty::Hard, player_level, kind),
        ];
        choices.push(DialogueChoice {
            id: "nevermind".to_string(),
            text: "Back to board".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("adventure_board_contract:{}:{}", npc_id, kind.as_str()),
                npc_id: npc_id.to_string(),
                speaker: ADVENTURE_BOARD_NAME.to_string(),
                text: format!(
                    "This section lists short {} jobs. Pick one contract and bring back the materials requested.",
                    kind.skill_name()
                ),
                choices,
            },
        )
        .await;
    }

    pub(in crate::game) async fn handle_accept_resource_contract(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
        difficulty_str: &str,
    ) {
        let Some(kind) = kind_for_entity_type(entity_type) else {
            self.send_system_message(player_id, "That NPC does not offer contracts.")
                .await;
            return;
        };
        let Some(difficulty) = ContractDifficulty::from_str(difficulty_str) else {
            self.send_system_message(player_id, "Invalid contract difficulty.")
                .await;
            return;
        };

        {
            let resource_contracts = self.resource_contracts.read().await;
            if resource_contracts.has_contract(player_id) {
                self.send_system_message(
                    player_id,
                    "You already have an active resource contract.",
                )
                .await;
                return;
            }
        }

        let Some(player_level) = self.player_skill_level_for_contract(player_id, kind).await else {
            return;
        };

        if player_level < difficulty.level_required() {
            self.send_system_message(
                player_id,
                &format!(
                    "You need {} level {} for {} contracts.",
                    kind.display_name(),
                    difficulty.level_required(),
                    difficulty.display_name()
                ),
            )
            .await;
            return;
        }

        let Some(prototype) = self.entity_registry.get(entity_type) else {
            return;
        };

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let contract = match self
            .generate_resource_contract(
                player_id,
                npc_id,
                &prototype.display_name,
                kind,
                difficulty,
                player_level,
                current_time,
            )
            .await
        {
            Ok(contract) => contract,
            Err(message) => {
                self.send_system_message(player_id, &message).await;
                return;
            }
        };

        {
            let mut resource_contracts = self.resource_contracts.write().await;
            if resource_contracts.has_contract(player_id) {
                self.send_system_message(
                    player_id,
                    "You already have an active resource contract.",
                )
                .await;
                return;
            }
            resource_contracts.insert_contract(contract.clone());
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db
                .save_resource_contract(
                    player_id,
                    contract.kind.as_str(),
                    contract.difficulty.as_str(),
                    &contract.target_item_id,
                    &contract.target_name,
                    contract.amount_required,
                    contract.amount_completed,
                    &contract.giver_npc_id,
                    &contract.giver_name,
                    contract.created_at,
                )
                .await
            {
                tracing::error!("Failed to save resource contract: {}", e);
            }
        }

        self.send_system_message(
            player_id,
            &format!(
                "Contract accepted! {} {} {}.",
                contract.kind.action_text(),
                contract.amount_required,
                contract.target_name
            ),
        )
        .await;
        self.send_resource_contract_update(player_id).await;
    }

    pub(in crate::game) async fn handle_accept_adventure_board_contract(
        &self,
        player_id: &str,
        npc_id: &str,
        kind_str: &str,
        difficulty_str: &str,
    ) {
        let Some(kind) = ResourceContractKind::from_str(kind_str) else {
            self.send_system_message(player_id, "Invalid board contract type.")
                .await;
            return;
        };
        let Some(difficulty) = ContractDifficulty::from_str(difficulty_str) else {
            self.send_system_message(player_id, "Invalid contract difficulty.")
                .await;
            return;
        };

        {
            let resource_contracts = self.resource_contracts.read().await;
            if resource_contracts.has_contract(player_id) {
                self.send_system_message(
                    player_id,
                    "You already have an active resource contract.",
                )
                .await;
                return;
            }
        }

        let Some(player_level) = self.player_skill_level_for_contract(player_id, kind).await else {
            return;
        };

        if player_level < difficulty.level_required() {
            self.send_system_message(
                player_id,
                &format!(
                    "You need {} level {} for {} contracts.",
                    kind.display_name(),
                    difficulty.level_required(),
                    difficulty.display_name()
                ),
            )
            .await;
            return;
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let contract = match self
            .generate_resource_contract(
                player_id,
                npc_id,
                ADVENTURE_BOARD_NAME,
                kind,
                difficulty,
                player_level,
                current_time,
            )
            .await
        {
            Ok(contract) => contract,
            Err(message) => {
                self.send_system_message(player_id, &message).await;
                return;
            }
        };

        {
            let mut resource_contracts = self.resource_contracts.write().await;
            if resource_contracts.has_contract(player_id) {
                self.send_system_message(
                    player_id,
                    "You already have an active resource contract.",
                )
                .await;
                return;
            }
            resource_contracts.insert_contract(contract.clone());
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db
                .save_resource_contract(
                    player_id,
                    contract.kind.as_str(),
                    contract.difficulty.as_str(),
                    &contract.target_item_id,
                    &contract.target_name,
                    contract.amount_required,
                    contract.amount_completed,
                    &contract.giver_npc_id,
                    &contract.giver_name,
                    contract.created_at,
                )
                .await
            {
                tracing::error!("Failed to save board resource contract: {}", e);
            }
        }

        self.send_system_message(
            player_id,
            &format!(
                "Board contract accepted! {} {} {}.",
                contract.kind.action_text(),
                contract.amount_required,
                contract.target_name
            ),
        )
        .await;
        self.send_resource_contract_update(player_id).await;
    }

    pub(in crate::game) async fn handle_claim_resource_contract(&self, player_id: &str) {
        let contract_status = {
            let resource_contracts = self.resource_contracts.read().await;
            match resource_contracts.get_contract(player_id) {
                Some(contract) if contract.is_complete() => ClaimContractStatus::Ready {
                    contract: contract.clone(),
                    xp_reward: contract.difficulty.xp_reward(contract.kind),
                    gold_reward: contract.difficulty.gold_reward(contract.kind),
                    seed_count: if contract.kind == ResourceContractKind::Farming {
                        contract.difficulty.farming_seed_reward_count()
                    } else {
                        0
                    },
                },
                Some(_) => ClaimContractStatus::Incomplete,
                None => ClaimContractStatus::Missing,
            }
        };

        let (contract, xp_reward, gold_reward, seed_count) = match contract_status {
            ClaimContractStatus::Ready {
                contract,
                xp_reward,
                gold_reward,
                seed_count,
            } => (contract, xp_reward, gold_reward, seed_count),
            ClaimContractStatus::Incomplete => {
                self.send_system_message(player_id, "Your contract isn't complete yet.")
                    .await;
                return;
            }
            ClaimContractStatus::Missing => {
                self.send_system_message(player_id, "You don't have an active contract.")
                    .await;
                return;
            }
        };

        let (inventory_update, gold, skill_name, total_xp, level, leveled_up, seed_names) = {
            let farming = self.farming.read().await;
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };

            player.inventory.gold += gold_reward;

            let mut seed_names = Vec::new();
            if contract.kind == ResourceContractKind::Farming {
                for _ in 0..seed_count {
                    if let Some(seed_id) =
                        farming.random_seed_for_level(player.skills.farming.level)
                    {
                        let seed_name = self
                            .item_registry
                            .get(&seed_id)
                            .map(|def| def.display_name.clone())
                            .unwrap_or_else(|| seed_id.clone());
                        player.inventory.add_item(&seed_id, 1, &self.item_registry);
                        seed_names.push(seed_name);
                    }
                }
            }

            let (skill_name, leveled_up, total_xp, level) = match contract.kind {
                ResourceContractKind::Farming => {
                    let leveled_up = player.skills.farming.add_xp(xp_reward);
                    (
                        "farming",
                        leveled_up,
                        player.skills.farming.xp,
                        player.skills.farming.level,
                    )
                }
                ResourceContractKind::Mining => {
                    let leveled_up = player.skills.mining.add_xp(xp_reward);
                    (
                        "mining",
                        leveled_up,
                        player.skills.mining.xp,
                        player.skills.mining.level,
                    )
                }
                ResourceContractKind::Woodcutting => {
                    let leveled_up = player.skills.woodcutting.add_xp(xp_reward);
                    (
                        "woodcutting",
                        leveled_up,
                        player.skills.woodcutting.xp,
                        player.skills.woodcutting.level,
                    )
                }
                ResourceContractKind::Fishing => {
                    let leveled_up = player.skills.fishing.add_xp(xp_reward);
                    (
                        "fishing",
                        leveled_up,
                        player.skills.fishing.xp,
                        player.skills.fishing.level,
                    )
                }
                ResourceContractKind::Smithing => {
                    let leveled_up = player.skills.smithing.add_xp(xp_reward);
                    (
                        "smithing",
                        leveled_up,
                        player.skills.smithing.xp,
                        player.skills.smithing.level,
                    )
                }
            };

            (
                player.inventory.to_update(),
                player.inventory.gold,
                skill_name.to_string(),
                total_xp,
                level,
                leveled_up,
                seed_names,
            )
        };

        {
            let mut resource_contracts = self.resource_contracts.write().await;
            if resource_contracts.remove_contract(player_id).is_none() {
                self.send_system_message(player_id, "You don't have an active contract.")
                    .await;
                return;
            }
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db.delete_resource_contract(player_id).await {
                tracing::error!("Failed to delete resource contract: {}", e);
            }
            if contract.kind == ResourceContractKind::Farming {
                if let Err(e) = db.delete_farming_contract(player_id).await {
                    tracing::warn!("Failed to delete legacy farming contract: {}", e);
                }
            }
            if let Err(e) = db
                .add_resource_contract_completion(player_id, gold_reward, xp_reward)
                .await
            {
                tracing::warn!("Failed to record resource contract totals: {}", e);
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        )
        .await;
        self.send_to_player(
            player_id,
            ServerMessage::SkillXp {
                player_id: player_id.to_string(),
                skill: skill_name.clone(),
                xp_gained: xp_reward,
                total_xp,
                level,
            },
        )
        .await;

        if leveled_up {
            self.broadcast_skill_level_up(player_id, &skill_name, level)
                .await;
            self.process_quest_progression_snapshot(player_id).await;
        }

        let bonus_text = if seed_names.is_empty() {
            String::new()
        } else {
            format!(" and {}", seed_names.join(", "))
        };

        self.send_system_message(
            player_id,
            &format!(
                "{} contract complete! Received {} {} XP, {}gp{}.",
                contract.difficulty.display_name(),
                xp_reward,
                contract.kind.display_name(),
                gold_reward,
                bonus_text
            ),
        )
        .await;
        self.send_resource_contract_update(player_id).await;
    }

    pub(in crate::game) async fn handle_abandon_resource_contract(&self, player_id: &str) {
        let removed_contract = {
            let mut resource_contracts = self.resource_contracts.write().await;
            resource_contracts.remove_contract(player_id)
        };

        let Some(contract) = removed_contract else {
            self.send_system_message(player_id, "You don't have an active contract.")
                .await;
            return;
        };

        if let Some(ref db) = self.db {
            if let Err(e) = db.delete_resource_contract(player_id).await {
                tracing::error!("Failed to delete resource contract: {}", e);
            }
            if contract.kind == ResourceContractKind::Farming {
                if let Err(e) = db.delete_farming_contract(player_id).await {
                    tracing::warn!("Failed to delete legacy farming contract: {}", e);
                }
            }
        }

        self.send_system_message(player_id, "Contract abandoned.")
            .await;
        self.send_resource_contract_update(player_id).await;
    }

    pub async fn get_resource_contract_message(&self, player_id: &str) -> ServerMessage {
        let resource_contracts = self.resource_contracts.read().await;
        resource_contract_message(resource_contracts.get_contract(player_id))
    }

    pub async fn send_resource_contract_update(&self, player_id: &str) {
        let msg = self.get_resource_contract_message(player_id).await;
        self.send_to_player(player_id, msg).await;
    }

    pub(in crate::game) async fn record_resource_contract_progress(
        &self,
        player_id: &str,
        item_id: &str,
        amount: i32,
    ) {
        let progress = {
            let mut resource_contracts = self.resource_contracts.write().await;
            resource_contracts
                .record_item_progress(player_id, item_id, amount)
                .and_then(|(completed, required, complete)| {
                    resource_contracts
                        .get_contract(player_id)
                        .cloned()
                        .map(|contract| (contract, completed, required, complete))
                })
        };

        let Some((contract, completed, required, complete)) = progress else {
            return;
        };

        if complete {
            self.send_system_message(
                player_id,
                &format!(
                    "Contract complete! ({}/{}) Return to {} to claim your rewards.",
                    completed, required, contract.giver_name
                ),
            )
            .await;
        } else {
            self.send_system_message(
                player_id,
                &format!(
                    "Contract progress: {}/{} {}.",
                    completed,
                    required,
                    contract.kind.progress_label()
                ),
            )
            .await;
        }

        if let Some(ref db) = self.db {
            if let Err(e) = db
                .update_resource_contract_progress(player_id, completed)
                .await
            {
                tracing::warn!("Failed to update resource contract progress: {}", e);
            }
            if contract.kind == ResourceContractKind::Farming {
                if let Err(e) = db
                    .update_farming_contract_progress(player_id, completed)
                    .await
                {
                    tracing::warn!("Failed to update legacy farming contract progress: {}", e);
                }
            }
        }

        self.send_resource_contract_update(player_id).await;
    }

    async fn player_skill_level_for_contract(
        &self,
        player_id: &str,
        kind: ResourceContractKind,
    ) -> Option<i32> {
        let players = self.players.read().await;
        players.get(player_id).map(|player| match kind {
            ResourceContractKind::Farming => player.skills.farming.level,
            ResourceContractKind::Mining => player.skills.mining.level,
            ResourceContractKind::Woodcutting => player.skills.woodcutting.level,
            ResourceContractKind::Fishing => player.skills.fishing.level,
            ResourceContractKind::Smithing => player.skills.smithing.level,
        })
    }

    async fn generate_resource_contract(
        &self,
        player_id: &str,
        npc_id: &str,
        giver_name: &str,
        kind: ResourceContractKind,
        difficulty: ContractDifficulty,
        player_level: i32,
        current_time: u64,
    ) -> Result<ResourceContract, String> {
        let targets = self
            .resource_contract_targets(kind, player_level, difficulty.minimum_target_level())
            .await;
        if targets.is_empty() {
            return Err(format!(
                "No {} contract targets are available for your level.",
                kind.skill_name()
            ));
        }

        let mut rng = rand::thread_rng();
        let target = &targets[rng.gen_range(0..targets.len())];
        let (min, max) = difficulty.target_amount_range(kind);

        Ok(ResourceContract {
            player_id: player_id.to_string(),
            kind,
            difficulty,
            target_item_id: target.item_id.clone(),
            target_name: target.target_name.clone(),
            amount_required: rng.gen_range(min..=max),
            amount_completed: 0,
            created_at: current_time,
            giver_npc_id: npc_id.to_string(),
            giver_name: giver_name.to_string(),
        })
    }

    async fn resource_contract_targets(
        &self,
        kind: ResourceContractKind,
        player_level: i32,
        minimum_target_level: i32,
    ) -> Vec<ContractTarget> {
        let mut preferred = Vec::new();
        let mut fallback = Vec::new();

        match kind {
            ResourceContractKind::Farming => {
                let farming = self.farming.read().await;
                for crop in farming.crops.values() {
                    if crop.level_required > player_level {
                        continue;
                    }
                    let target = ContractTarget {
                        item_id: crop.produce_item.clone(),
                        target_name: self.display_name_for_item(&crop.produce_item),
                        level_required: crop.level_required,
                    };
                    if target.level_required >= minimum_target_level {
                        preferred.push(target);
                    } else {
                        fallback.push(target);
                    }
                }
            }
            ResourceContractKind::Mining => {
                let mining = self.mining.read().await;
                for ore in mining.ore_types.values() {
                    if ore.level_required > player_level {
                        continue;
                    }
                    let target = ContractTarget {
                        item_id: ore.ore_item_id.clone(),
                        target_name: self.display_name_for_item(&ore.ore_item_id),
                        level_required: ore.level_required,
                    };
                    if target.level_required >= minimum_target_level {
                        preferred.push(target);
                    } else {
                        fallback.push(target);
                    }
                }
            }
            ResourceContractKind::Woodcutting => {
                let woodcutting = self.woodcutting.read().await;
                for tree in woodcutting.tree_types.values() {
                    if tree.level_required > player_level {
                        continue;
                    }
                    let target = ContractTarget {
                        item_id: tree.log_item_id.clone(),
                        target_name: self.display_name_for_item(&tree.log_item_id),
                        level_required: tree.level_required,
                    };
                    if target.level_required >= minimum_target_level {
                        preferred.push(target);
                    } else {
                        fallback.push(target);
                    }
                }
            }
            ResourceContractKind::Fishing => {
                let gathering = self.gathering.read().await;
                let mut fishing_targets: HashMap<String, i32> = HashMap::new();
                for table in gathering.loot_tables.values() {
                    if table.skill != "fishing" {
                        continue;
                    }
                    for tier in table.tiers.values() {
                        for item in &tier.items {
                            if item.level > player_level {
                                continue;
                            }
                            fishing_targets
                                .entry(item.id.clone())
                                .and_modify(|level| *level = (*level).min(item.level))
                                .or_insert(item.level);
                        }
                    }
                }
                drop(gathering);

                for (item_id, level_required) in fishing_targets {
                    let target = ContractTarget {
                        target_name: self.display_name_for_item(&item_id),
                        item_id,
                        level_required,
                    };
                    if target.level_required >= minimum_target_level {
                        preferred.push(target);
                    } else {
                        fallback.push(target);
                    }
                }
            }
            ResourceContractKind::Smithing => {
                let mut smithing_targets: HashMap<String, i32> = HashMap::new();
                for recipe in self.crafting_registry.all() {
                    if recipe.category != crate::crafting::definition::RecipeCategory::Smithing {
                        continue;
                    }
                    if recipe.level_required > player_level || recipe.requires_discovery {
                        continue;
                    }
                    if recipe.results.len() != 1 {
                        continue;
                    }
                    let result = &recipe.results[0];
                    if result.count != 1 {
                        continue;
                    }
                    smithing_targets
                        .entry(result.item_id.clone())
                        .and_modify(|level| *level = (*level).min(recipe.level_required))
                        .or_insert(recipe.level_required);
                }

                for (item_id, level_required) in smithing_targets {
                    let target = ContractTarget {
                        item_id: item_id.clone(),
                        target_name: self.display_name_for_item(&item_id),
                        level_required,
                    };
                    if target.level_required >= minimum_target_level {
                        preferred.push(target);
                    } else {
                        fallback.push(target);
                    }
                }
            }
        }

        if preferred.is_empty() {
            fallback
        } else {
            preferred
        }
    }

    fn display_name_for_item(&self, item_id: &str) -> String {
        self.item_registry
            .get(item_id)
            .map(|item| item.display_name.clone())
            .unwrap_or_else(|| item_id.to_string())
    }
}
