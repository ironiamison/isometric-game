use super::*;
use crate::protocol::{SlayerRewardData, SlayerTaskData};

#[derive(Debug, Clone, PartialEq, Eq)]
enum RewardPurchasePlan {
    UnlockMonster(String),
    BlockMonster(String),
    GrantItem { item_id: String, quantity: i32 },
    NoEffect,
}

fn slayer_result(
    success: bool,
    action: &str,
    message: impl Into<String>,
    task: Option<SlayerTaskData>,
    points: Option<i32>,
) -> ServerMessage {
    ServerMessage::SlayerResult {
        success,
        action: action.to_string(),
        message: message.into(),
        task,
        points,
    }
}

fn slayer_task_data_from_state(state: &crate::slayer::PlayerSlayerState) -> Option<SlayerTaskData> {
    state.current_task.as_ref().map(|task| SlayerTaskData {
        monster_id: task.monster_id.clone(),
        display_name: task.display_name.clone(),
        kills_current: task.kills_current,
        kills_required: task.kills_required,
        xp_per_kill: task.xp_per_kill,
        master_id: task.master_id.clone(),
        points_on_complete: task.points_on_complete,
    })
}

fn slayer_reward_data(reward: &crate::slayer::SlayerRewardDef) -> SlayerRewardData {
    SlayerRewardData {
        id: reward.id.clone(),
        display_name: reward.display_name.clone(),
        description: reward.description.clone(),
        cost: reward.cost,
        category: reward.category.clone(),
        target_id: reward.target_id.clone(),
        quantity: reward.quantity,
    }
}

fn task_matches_kill(task_monster_id: &str, prototype_id: &str, aliases: &[String]) -> bool {
    task_monster_id == prototype_id
        || prototype_id.starts_with(&format!("{}_", task_monster_id))
        || aliases.iter().any(|a| a == prototype_id)
}

fn plan_reward_purchase(
    state: &crate::slayer::PlayerSlayerState,
    reward: &crate::slayer::SlayerRewardDef,
    target_monster_id: Option<&str>,
) -> Result<RewardPurchasePlan, String> {
    if state.points < reward.cost {
        return Err(format!("You need {} slayer points.", reward.cost));
    }

    match reward.category.as_str() {
        "unlock" => match reward.target_id.as_deref() {
            Some(target)
                if state
                    .unlocked_monsters
                    .iter()
                    .any(|monster| monster == target) =>
            {
                Err("Already unlocked.".to_string())
            }
            Some(target) => Ok(RewardPurchasePlan::UnlockMonster(target.to_string())),
            None => Ok(RewardPurchasePlan::NoEffect),
        },
        "block" => {
            let monster = target_monster_id.unwrap_or_default();
            if monster.is_empty() {
                Err("Select a monster to block.".to_string())
            } else if state
                .blocked_monsters
                .iter()
                .any(|blocked| blocked == monster)
            {
                Err("Already blocked.".to_string())
            } else {
                Ok(RewardPurchasePlan::BlockMonster(monster.to_string()))
            }
        }
        "potion" | "equipment" => match reward.target_id.as_deref() {
            Some(item_id) => Ok(RewardPurchasePlan::GrantItem {
                item_id: item_id.to_string(),
                quantity: reward.quantity,
            }),
            None => Ok(RewardPurchasePlan::NoEffect),
        },
        _ => Ok(RewardPurchasePlan::NoEffect),
    }
}

fn apply_reward_purchase(
    state: &mut crate::slayer::PlayerSlayerState,
    reward_cost: i32,
    plan: &RewardPurchasePlan,
) {
    match plan {
        RewardPurchasePlan::UnlockMonster(monster_id) => {
            state.points -= reward_cost;
            state.unlocked_monsters.push(monster_id.clone());
        }
        RewardPurchasePlan::BlockMonster(monster_id) => {
            state.points -= reward_cost;
            state.blocked_monsters.push(monster_id.clone());
        }
        RewardPurchasePlan::GrantItem { .. } => {
            state.points -= reward_cost;
        }
        RewardPurchasePlan::NoEffect => {}
    }
}

impl GameRoom {
    pub async fn handle_slayer_master_interact(&self, player_id: &str, npc_prototype_id: &str) {
        let master = match self
            .slayer_registry
            .get_master_by_prototype(npc_prototype_id)
        {
            Some(master) => master,
            None => return,
        };

        let state = self.get_player_slayer_state(player_id).await;
        let rewards: Vec<SlayerRewardData> = self
            .slayer_registry
            .get_rewards()
            .iter()
            .map(slayer_reward_data)
            .collect();

        self.send_to_player(
            player_id,
            ServerMessage::SlayerPanelOpen {
                master_id: master.id.clone(),
                master_name: master.display_name.clone(),
                current_task: slayer_task_data_from_state(&state),
                points: state.points,
                tasks_completed: state.tasks_completed,
                rewards,
                blocked_monsters: state.blocked_monsters.clone(),
                unlocked_monsters: state.unlocked_monsters.clone(),
                blockable_monsters: self.slayer_registry.get_all_blockable_monsters(),
            },
        )
        .await;
    }

    pub async fn handle_slayer_get_task(&self, player_id: &str, master_id: &str) {
        let master = match self.slayer_registry.get_master(master_id) {
            Some(master) => master,
            None => {
                self.send_to_player(
                    player_id,
                    slayer_result(false, "get_task", "Unknown slayer master.", None, None),
                )
                .await;
                return;
            }
        };

        let combat_level = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|player| player.skills.combat_level())
                .unwrap_or(0)
        };
        if combat_level < master.combat_level_required {
            self.send_to_player(
                player_id,
                slayer_result(
                    false,
                    "get_task",
                    format!(
                        "You need combat level {} to get tasks from {}.",
                        master.combat_level_required, master.display_name
                    ),
                    None,
                    None,
                ),
            )
            .await;
            return;
        }

        let slayer_level = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|player| player.skills.slayer.level)
                .unwrap_or(1)
        };
        if slayer_level < master.slayer_level_required {
            self.send_to_player(
                player_id,
                slayer_result(
                    false,
                    "get_task",
                    format!(
                        "You need slayer level {} to get tasks from {}.",
                        master.slayer_level_required, master.display_name
                    ),
                    None,
                    None,
                ),
            )
            .await;
            return;
        }

        let mut state = self.get_player_slayer_state(player_id).await;
        if state.current_task.is_some() {
            self.send_to_player(
                player_id,
                slayer_result(
                    false,
                    "get_task",
                    "You already have an active slayer task.",
                    None,
                    None,
                ),
            )
            .await;
            return;
        }

        match self
            .slayer_registry
            .assign_task(master_id, slayer_level, &state)
        {
            Some(task) => {
                let task_data = SlayerTaskData {
                    monster_id: task.monster_id.clone(),
                    display_name: task.display_name.clone(),
                    kills_current: task.kills_current,
                    kills_required: task.kills_required,
                    xp_per_kill: task.xp_per_kill,
                    master_id: task.master_id.clone(),
                    points_on_complete: task.points_on_complete,
                };
                state.current_task = Some(task);
                self.set_player_slayer_state(player_id, state).await;

                self.send_to_player(
                    player_id,
                    slayer_result(
                        true,
                        "get_task",
                        format!(
                            "New task: Slay {} {}.",
                            task_data.kills_required, task_data.display_name
                        ),
                        Some(task_data),
                        None,
                    ),
                )
                .await;
            }
            None => {
                self.send_to_player(
                    player_id,
                    slayer_result(
                        false,
                        "get_task",
                        "No eligible tasks available.",
                        None,
                        None,
                    ),
                )
                .await;
            }
        }
    }

    pub async fn handle_slayer_cancel_task(&self, player_id: &str) {
        let mut state = self.get_player_slayer_state(player_id).await;

        if state.current_task.is_none() {
            self.send_to_player(
                player_id,
                slayer_result(
                    false,
                    "cancel_task",
                    "You don't have an active task.",
                    None,
                    None,
                ),
            )
            .await;
            return;
        }

        if state.points < 30 {
            self.send_to_player(
                player_id,
                slayer_result(
                    false,
                    "cancel_task",
                    "You need 30 slayer points to cancel a task.",
                    None,
                    Some(state.points),
                ),
            )
            .await;
            return;
        }

        state.points -= 30;
        state.current_task = None;
        self.set_player_slayer_state(player_id, state.clone()).await;

        self.send_to_player(
            player_id,
            slayer_result(
                true,
                "cancel_task",
                "Task cancelled. 30 points deducted.",
                None,
                Some(state.points),
            ),
        )
        .await;
    }

    pub async fn process_slayer_kill(&self, player_id: &str, prototype_id: &str) {
        let mut state = self.get_player_slayer_state(player_id).await;
        let task = match &mut state.current_task {
            Some(task) if task_matches_kill(&task.monster_id, prototype_id, &task.aliases) => task,
            _ => return,
        };

        task.kills_current += 1;
        let xp_per_kill = task.xp_per_kill;
        let kills_current = task.kills_current;
        let kills_required = task.kills_required;
        let display_name = task.display_name.clone();
        let monster_id = task.monster_id.clone();

        let xp_result = {
            let mut players = self.players.write().await;
            players.get_mut(player_id).map(|player| {
                let leveled_up = player.skills.slayer.add_xp(xp_per_kill);
                (
                    player.skills.slayer.xp,
                    player.skills.slayer.level,
                    leveled_up,
                )
            })
        };

        if let Some((total_xp, level, leveled_up)) = xp_result {
            self.send_to_player(
                player_id,
                ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: "slayer".to_string(),
                    xp_gained: xp_per_kill,
                    total_xp,
                    level,
                },
            )
            .await;

            if leveled_up {
                self.broadcast_skill_level_up(player_id, "slayer", level).await;
            }
        }

        if kills_current >= kills_required {
            let points_awarded = task.points_on_complete;
            state.points += points_awarded;
            state.tasks_completed += 1;
            state.current_task = None;
            self.set_player_slayer_state(player_id, state.clone()).await;

            self.send_to_player(
                player_id,
                ServerMessage::SlayerTaskComplete {
                    monster_id,
                    display_name,
                    points_awarded,
                    total_points: state.points,
                },
            )
            .await;

            tracing::info!(
                "Player {} completed slayer task, earned {} points (total: {})",
                player_id,
                points_awarded,
                state.points
            );
        } else {
            self.set_player_slayer_state(player_id, state).await;

            self.send_to_player(
                player_id,
                ServerMessage::SlayerTaskProgress {
                    monster_id,
                    display_name,
                    kills_current,
                    kills_required,
                },
            )
            .await;
        }
    }

    pub async fn handle_slayer_buy_reward(
        &self,
        player_id: &str,
        reward_id: &str,
        target_monster_id: Option<String>,
    ) {
        let reward = match self.slayer_registry.get_reward(reward_id) {
            Some(reward) => reward.clone(),
            None => {
                self.send_to_player(
                    player_id,
                    slayer_result(false, "buy_reward", "Unknown reward.", None, None),
                )
                .await;
                return;
            }
        };

        let mut state = self.get_player_slayer_state(player_id).await;
        let plan = match plan_reward_purchase(&state, &reward, target_monster_id.as_deref()) {
            Ok(plan) => plan,
            Err(message) => {
                self.send_to_player(
                    player_id,
                    slayer_result(false, "buy_reward", message, None, Some(state.points)),
                )
                .await;
                return;
            }
        };

        if let RewardPurchasePlan::GrantItem { item_id, quantity } = &plan {
            let inv_update = {
                let mut players = self.players.write().await;
                if let Some(player) = players.get_mut(player_id) {
                    if !player
                        .inventory
                        .has_space_for(item_id, *quantity, &self.item_registry)
                    {
                        self.send_to_player(
                            player_id,
                            slayer_result(
                                false,
                                "buy_reward",
                                "Your inventory is full.",
                                None,
                                Some(state.points),
                            ),
                        )
                        .await;
                        return;
                    }

                    player
                        .inventory
                        .add_item(item_id, *quantity, &self.item_registry);
                    Some((player.inventory.to_update(), player.inventory.gold))
                } else {
                    None
                }
            };

            if let Some((slots, gold)) = inv_update {
                self.send_to_player(
                    player_id,
                    ServerMessage::InventoryUpdate {
                        player_id: player_id.to_string(),
                        slots,
                        gold,
                    },
                )
                .await;
            }
        }

        apply_reward_purchase(&mut state, reward.cost, &plan);
        self.set_player_slayer_state(player_id, state.clone()).await;

        self.send_to_player(
            player_id,
            slayer_result(
                true,
                "buy_reward",
                format!("Purchased {}.", reward.display_name),
                None,
                Some(state.points),
            ),
        )
        .await;
    }

    pub async fn handle_slayer_remove_block(&self, player_id: &str, monster_id: &str) {
        let mut state = self.get_player_slayer_state(player_id).await;

        if let Some(pos) = state
            .blocked_monsters
            .iter()
            .position(|monster| monster == monster_id)
        {
            state.blocked_monsters.remove(pos);
            self.set_player_slayer_state(player_id, state.clone()).await;

            self.send_to_player(
                player_id,
                slayer_result(
                    true,
                    "remove_block",
                    "Block removed.",
                    None,
                    Some(state.points),
                ),
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reward(
        id: &str,
        cost: i32,
        category: &str,
        target_id: Option<&str>,
        quantity: i32,
    ) -> crate::slayer::SlayerRewardDef {
        crate::slayer::SlayerRewardDef {
            id: id.to_string(),
            display_name: id.to_string(),
            description: "reward".to_string(),
            cost,
            category: category.to_string(),
            target_id: target_id.map(str::to_string),
            quantity,
        }
    }

    #[test]
    fn task_matches_kill_accepts_exact_and_variant_ids() {
        assert!(task_matches_kill("goblin", "goblin", &[]));
        assert!(task_matches_kill("goblin", "goblin_champion", &[]));
        assert!(!task_matches_kill("goblin", "skeleton", &[]));
    }

    #[test]
    fn task_matches_kill_accepts_aliases() {
        let aliases = vec!["piglet".to_string()];
        assert!(task_matches_kill("pig", "pig", &aliases));
        assert!(task_matches_kill("pig", "piglet", &aliases));
        assert!(task_matches_kill("pig", "pig_king", &aliases));
        assert!(!task_matches_kill("pig", "skeleton", &aliases));
    }

    #[test]
    fn plan_reward_purchase_validates_duplicates_and_block_targets() {
        let mut state = crate::slayer::PlayerSlayerState {
            points: 100,
            ..Default::default()
        };
        state.unlocked_monsters.push("abyssal_demon".to_string());
        state.blocked_monsters.push("cave_crawler".to_string());

        assert_eq!(
            plan_reward_purchase(
                &state,
                &reward("unlock_demon", 50, "unlock", Some("abyssal_demon"), 1),
                None
            ),
            Err("Already unlocked.".to_string())
        );
        assert_eq!(
            plan_reward_purchase(&state, &reward("block", 25, "block", None, 1), None),
            Err("Select a monster to block.".to_string())
        );
        assert_eq!(
            plan_reward_purchase(
                &state,
                &reward("block", 25, "block", None, 1),
                Some("cave_crawler")
            ),
            Err("Already blocked.".to_string())
        );
    }

    #[test]
    fn apply_reward_purchase_updates_points_and_lists_for_unlocks_and_blocks() {
        let mut state = crate::slayer::PlayerSlayerState {
            points: 90,
            ..Default::default()
        };

        apply_reward_purchase(
            &mut state,
            40,
            &RewardPurchasePlan::UnlockMonster("abyssal_demon".to_string()),
        );
        assert_eq!(state.points, 50);
        assert_eq!(state.unlocked_monsters, vec!["abyssal_demon"]);

        apply_reward_purchase(
            &mut state,
            20,
            &RewardPurchasePlan::BlockMonster("cave_crawler".to_string()),
        );
        assert_eq!(state.points, 30);
        assert_eq!(state.blocked_monsters, vec!["cave_crawler"]);

        apply_reward_purchase(
            &mut state,
            10,
            &RewardPurchasePlan::GrantItem {
                item_id: "slayer_helmet".to_string(),
                quantity: 1,
            },
        );
        assert_eq!(state.points, 20);
    }
}
