use super::*;
use crate::protocol::DialogueChoice;

enum ClaimContractStatus {
    Ready {
        xp_reward: i64,
        gold_reward: i32,
        seed_count: i32,
        diff_name: String,
    },
    Incomplete,
    Missing,
}

impl GameRoom {
    pub(in crate::game) async fn show_master_farmer_dialogue(&self, player_id: &str, npc_id: &str) {
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

    pub(in crate::game) async fn show_contract_dialogue(&self, player_id: &str, npc_id: &str) {
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

    pub(in crate::game) async fn handle_accept_contract(
        &self,
        player_id: &str,
        difficulty_str: &str,
    ) {
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

    pub(in crate::game) async fn handle_claim_contract(&self, player_id: &str) {
        let contract_status = {
            let farming = self.farming.read().await;
            match farming.get_contract(player_id) {
                Some(contract) if contract.is_complete() => ClaimContractStatus::Ready {
                    xp_reward: contract.difficulty.xp_reward(),
                    gold_reward: contract.difficulty.gold_reward(),
                    seed_count: contract.difficulty.seed_reward_count(),
                    diff_name: contract.difficulty.display_name().to_string(),
                },
                Some(_) => ClaimContractStatus::Incomplete,
                None => ClaimContractStatus::Missing,
            }
        };

        let (xp_reward, gold_reward, seed_count, diff_name) = match contract_status {
            ClaimContractStatus::Ready {
                xp_reward,
                gold_reward,
                seed_count,
                diff_name,
            } => (xp_reward, gold_reward, seed_count, diff_name),
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
            self.broadcast_skill_level_up(player_id, "farming", skill_level).await;
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

    pub(in crate::game) async fn handle_abandon_contract(&self, player_id: &str) {
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
}
