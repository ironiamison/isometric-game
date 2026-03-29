use super::GameRoom;
use crate::protocol::{DialogueChoice, QuestObjectiveData, ServerMessage};
use crate::quest::runner::{DialogueChoice as QuestDialogueChoice, ScriptResult};
use crate::quest::{ObjectiveType, PlayerQuestState, Quest, QuestEvent, QuestStatus};
use std::sync::Arc;

fn quest_progress_message(
    quest_id: &str,
    objective_id: &str,
    current: i32,
    target: i32,
) -> ServerMessage {
    ServerMessage::QuestObjectiveProgress {
        quest_id: quest_id.to_string(),
        objective_id: objective_id.to_string(),
        current,
        target,
    }
}

fn player_progression_snapshot(player: &super::Player) -> (i32, Vec<(&'static str, i32)>) {
    (
        player.inventory.gold,
        vec![
            ("hitpoints", player.skills.hitpoints.level),
            ("attack", player.skills.attack.level),
            ("strength", player.skills.strength.level),
            ("defence", player.skills.defence.level),
            ("fishing", player.skills.fishing.level),
            ("farming", player.skills.farming.level),
            ("smithing", player.skills.smithing.level),
            ("prayer", player.skills.prayer.level),
            ("magic", player.skills.magic.level),
            ("woodcutting", player.skills.woodcutting.level),
            ("mining", player.skills.mining.level),
            ("alchemy", player.skills.alchemy.level),
            ("combat", player.skills.combat_level()),
        ],
    )
}

fn greeting_dialogue_metadata(entity_type: &str) -> (String, Vec<DialogueChoice>) {
    if entity_type == "old_thomas" {
        (
            "__tutorial__".to_string(),
            vec![
                DialogueChoice {
                    id: "accept".to_string(),
                    text: "Yes, show me around!".to_string(),
                },
                DialogueChoice {
                    id: "skip".to_string(),
                    text: "No thanks, I'll figure it out.".to_string(),
                },
            ],
        )
    } else {
        (
            String::new(),
            vec![DialogueChoice {
                id: "close".to_string(),
                text: "Goodbye".to_string(),
            }],
        )
    }
}

fn script_dialogue_choices(choices: Vec<QuestDialogueChoice>) -> Vec<DialogueChoice> {
    choices
        .into_iter()
        .map(|choice| DialogueChoice {
            id: choice.id,
            text: choice.text,
        })
        .collect()
}

fn select_target_quest(
    quests: &[Arc<Quest>],
    quest_state: &mut PlayerQuestState,
) -> Option<(String, &'static str)> {
    let mut target_quest: Option<(String, &'static str)> = None;
    let mut completed_quest: Option<(String, &'static str)> = None;

    for quest in quests {
        let quest_id = &quest.id;

        if let Some(progress) = quest_state.get_quest(quest_id) {
            if progress.status == QuestStatus::ReadyToComplete {
                target_quest = Some((quest_id.clone(), "ready_to_complete"));
                break;
            } else if progress.status == QuestStatus::Active {
                target_quest = Some((quest_id.clone(), "in_progress"));
            }
        } else if quest_state.is_quest_completed(quest_id) {
            if quest.repeatable {
                if target_quest.is_none() {
                    quest_state.completed_quests.retain(|id| id != quest_id);
                    target_quest = Some((quest_id.clone(), "not_started"));
                }
            } else if completed_quest.is_none() {
                completed_quest = Some((quest_id.clone(), "completed"));
            }
        } else {
            if let Some(ref previous) = quest.chain.previous {
                if !quest_state.is_quest_completed(previous) {
                    continue;
                }
            }
            if target_quest.is_none() {
                target_quest = Some((quest_id.clone(), "not_started"));
            }
        }
    }

    target_quest.or(completed_quest)
}

impl GameRoom {
    async fn send_quest_progress_update(
        &self,
        player_id: &str,
        quest_id: &str,
        objective_id: &str,
        current: i32,
        target: i32,
    ) {
        if let Some(sender) = self.player_senders.read().await.get(player_id) {
            let msg = quest_progress_message(quest_id, objective_id, current, target);
            if let Ok(data) = crate::protocol::encode_server_message(&msg) {
                let _ = sender.send(data).await;
            }
        }
    }

    async fn send_script_dialogue(
        &self,
        player_id: &str,
        quest_id: &str,
        npc_id: &str,
        quest_state: &mut PlayerQuestState,
        script_result: &ScriptResult,
        close_if_none: bool,
    ) {
        if let Some(dialogue) = script_result.dialogue.as_ref() {
            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: quest_id.to_string(),
                    npc_id: npc_id.to_string(),
                    speaker: dialogue.speaker.clone(),
                    text: dialogue.text.clone(),
                    choices: script_dialogue_choices(dialogue.choices.clone()),
                },
            )
            .await;

            if let Some(step) = script_result.new_dialogue_step {
                let step_key = format!("{}_dialogue_step", quest_id);
                quest_state.set_flag(&step_key, &step.to_string());
            }
        } else if close_if_none {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;

            let step_key = format!("{}_dialogue_step", quest_id);
            quest_state.flags.remove(&step_key);
        }
    }

    async fn accept_quest_with_snapshot_progress(
        &self,
        player_id: &str,
        quest_id: &str,
        quest_state: &mut PlayerQuestState,
    ) {
        if let Some(quest) = self.quest_registry.get(quest_id).await {
            let objective_targets: Vec<(String, i32)> = quest
                .objectives
                .iter()
                .map(|objective| (objective.id.clone(), objective.count))
                .collect();
            quest_state.start_quest(quest_id, &objective_targets);
            tracing::info!("Player {} accepted quest {}", player_id, quest_id);

            if let Some((gold, skill_levels)) = {
                let players = self.players.read().await;
                players.get(player_id).map(player_progression_snapshot)
            } {
                for (skill, level) in &skill_levels {
                    let event = QuestEvent::SkillLevelChanged {
                        player_id: player_id.to_string(),
                        skill: (*skill).to_string(),
                        level: *level,
                    };
                    self.quest_registry.process_event(&event, quest_state).await;
                }
                let gold_event = QuestEvent::GoldAmountChanged {
                    player_id: player_id.to_string(),
                    amount: gold,
                };
                self.quest_registry
                    .process_event(&gold_event, quest_state)
                    .await;
            }

            let objectives: Vec<QuestObjectiveData> = quest
                .objectives
                .iter()
                .map(|objective| {
                    let (current, completed) = quest_state
                        .get_quest(quest_id)
                        .and_then(|progress| progress.objectives.get(&objective.id))
                        .map(|progress| (progress.current, progress.completed))
                        .unwrap_or((0, false));
                    QuestObjectiveData {
                        id: objective.id.clone(),
                        description: objective.description.clone(),
                        current,
                        target: objective.count,
                        completed,
                    }
                })
                .collect();

            self.send_to_player(
                player_id,
                ServerMessage::QuestAccepted {
                    quest_id: quest_id.to_string(),
                    quest_name: quest.name.clone(),
                    objectives,
                },
            )
            .await;
        }
    }

    async fn complete_quest_and_grant_rewards(
        &self,
        player_id: &str,
        quest_id: &str,
        quest_state: &mut PlayerQuestState,
    ) {
        if let Some(quest) = self.quest_registry.get(quest_id).await {
            // Verify the player has all required collect items in their inventory
            // before completing (items may have been banked after collection).
            // Skip objectives with consume=false (intermediate items used up before turn-in).
            {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    for objective in &quest.objectives {
                        if objective.objective_type == ObjectiveType::CollectItem
                            && objective.consume
                            && !player.inventory.has_item(&objective.target, objective.count)
                        {
                            tracing::warn!(
                                "Player {} tried to complete quest {} but missing {} x{} in inventory",
                                player_id, quest_id, objective.target, objective.count
                            );
                            return;
                        }
                    }
                }
            }

            quest_state.complete_quest(quest_id);
            self.send_to_player(
                player_id,
                ServerMessage::QuestCompleted {
                    quest_id: quest_id.to_string(),
                    quest_name: quest.name.clone(),
                    rewards_exp: quest.rewards.exp,
                    rewards_gold: quest.rewards.gold,
                },
            )
            .await;

            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                for objective in &quest.objectives {
                    if objective.objective_type == ObjectiveType::CollectItem && objective.consume {
                        player
                            .inventory
                            .remove_item(&objective.target, objective.count);
                    }
                }

                player.inventory.gold += quest.rewards.gold;
                for item_reward in &quest.rewards.items {
                    player.inventory.add_item(
                        &item_reward.item_id,
                        item_reward.count,
                        &self.item_registry,
                    );
                }
                let slots = player.inventory.to_update();
                let gold = player.inventory.gold;
                drop(players);
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
        tracing::info!("Player {} completed quest {}", player_id, quest_id);
    }

    async fn grant_script_items(
        &self,
        player_id: &str,
        granted_items: &[(String, i32)],
        quest_state: &mut PlayerQuestState,
    ) {
        if granted_items.is_empty() {
            return;
        }

        let mut players = self.players.write().await;
        if let Some(player) = players.get_mut(player_id) {
            for (item_id, count) in granted_items {
                player
                    .inventory
                    .add_item(item_id, *count, &self.item_registry);
            }
            let slots = player.inventory.to_update();
            let gold = player.inventory.gold;
            drop(players);
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots,
                    gold,
                },
            )
            .await;

            // Fire ItemCollected events so collect_item objectives progress.
            // Use the already-held quest_state to avoid deadlocking on the lock.
            for (item_id, count) in granted_items {
                let event = QuestEvent::ItemCollected {
                    player_id: player_id.to_string(),
                    item_id: item_id.to_string(),
                    count: *count,
                };
                let results = self.quest_registry.process_event(&event, quest_state).await;
                for result in results {
                    if let (Some(objective_id), Some(current), Some(target)) =
                        (&result.objective_id, result.new_progress, result.target)
                    {
                        self.send_quest_progress_update(
                            player_id,
                            &result.quest_id,
                            objective_id,
                            current,
                            target,
                        )
                        .await;
                    }
                }
            }
        }
    }

    pub(in crate::game) async fn handle_npc_quest_interaction(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
    ) {
        let prototype = self.entity_registry.get(entity_type);
        // Get quests where this NPC is the giver_npc in the quest TOML
        let mut quests = self.quest_registry.get_quests_for_npc(entity_type).await;
        // Also include quests listed in the NPC prototype's available_quests
        // (for secondary NPCs like Barnaby or searchable objects like bookshelves)
        if let Some(ref proto) = prototype {
            if let Some(ref qg) = proto.quest_giver {
                let existing: std::collections::HashSet<String> =
                    quests.iter().map(|q| q.id.clone()).collect();
                for quest_id in &qg.available_quests {
                    if !existing.contains(quest_id.as_str()) {
                        if let Some(quest) = self.quest_registry.get(quest_id).await {
                            quests.push(quest);
                        }
                    }
                }
            }
        }

        if quests.is_empty() {
            tracing::debug!("NPC {} ({}) has no quests", npc_id, entity_type);
            if let Some(proto) = prototype.as_ref() {
                if let Some(greeting) = proto.dialogue.greeting.as_ref() {
                    let (quest_id, choices) = greeting_dialogue_metadata(entity_type);
                    self.send_to_player(
                        player_id,
                        ServerMessage::ShowDialogue {
                            quest_id,
                            npc_id: npc_id.to_string(),
                            speaker: proto.display_name.clone(),
                            text: greeting.clone(),
                            choices,
                        },
                    )
                    .await;
                }
            }
            return;
        }

        self.process_quest_progression_snapshot(player_id).await;

        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let mut talk_objective_dialogue: Option<String> = None;
        {
            let event = QuestEvent::NpcInteraction {
                player_id: player_id.to_string(),
                npc_id: entity_type.to_string(),
            };
            let talk_results = self.quest_registry.process_event(&event, quest_state).await;
            for result in talk_results {
                if let (Some(objective_id), Some(current), Some(target)) =
                    (&result.objective_id, result.new_progress, result.target)
                {
                    tracing::info!(
                        "Player {} talked to NPC for quest objective {} in quest {}: {}/{}",
                        player_id,
                        objective_id,
                        result.quest_id,
                        current,
                        target
                    );

                    self.send_quest_progress_update(
                        player_id,
                        &result.quest_id,
                        objective_id,
                        current,
                        target,
                    )
                    .await;

                    if result.objective_completed {
                        if let Some(quest) = self.quest_registry.get(&result.quest_id).await {
                            if let Some(objective) =
                                quest.objectives.iter().find(|o| o.id == *objective_id)
                            {
                                if let Some(dialogue) = objective.dialogue.as_ref() {
                                    talk_objective_dialogue = Some(dialogue.clone());
                                }
                            }
                        }
                    }
                }

                if result.quest_ready {
                    tracing::info!(
                        "Player {} quest {} is now ready to complete after talking to NPC",
                        player_id,
                        result.quest_id
                    );
                }
            }
        }

        let target_quest = select_target_quest(&quests, quest_state);

        if let Some((quest_id, state)) = target_quest {
            tracing::info!(
                "Player {} interacting with quest {} (state: {})",
                player_id,
                quest_id,
                state
            );

            match self
                .quest_runner
                .run_on_interact(player_id, &quest_id, quest_state, None, Some(entity_type))
                .await
            {
                Ok(script_result) => {
                    self.send_script_dialogue(
                        player_id,
                        &quest_id,
                        npc_id,
                        quest_state,
                        &script_result,
                        false,
                    )
                    .await;

                    if script_result.quest_accepted {
                        self.accept_quest_with_snapshot_progress(player_id, &quest_id, quest_state)
                            .await;

                        // Fire NpcInteraction for the NPC we're talking to so the
                        // giver's talk_to objective auto-completes on acceptance
                        // (the event fired earlier when the quest wasn't active yet)
                        let accept_event = QuestEvent::NpcInteraction {
                            player_id: player_id.to_string(),
                            npc_id: entity_type.to_string(),
                        };
                        let results = self.quest_registry.process_event(&accept_event, quest_state).await;
                        for result in results {
                            if let (Some(objective_id), Some(current), Some(target)) =
                                (&result.objective_id, result.new_progress, result.target)
                            {
                                self.send_quest_progress_update(
                                    player_id,
                                    &result.quest_id,
                                    objective_id,
                                    current,
                                    target,
                                )
                                .await;
                            }
                        }
                    }

                    if script_result.quest_completed {
                        self.complete_quest_and_grant_rewards(player_id, &quest_id, quest_state)
                            .await;
                    }

                    self.grant_script_items(player_id, &script_result.granted_items, quest_state)
                        .await;

                    for notification in script_result.notifications {
                        tracing::info!("Quest notification for {}: {}", player_id, notification);
                    }
                }
                Err(error) => {
                    tracing::error!("Quest script error: {}", error);
                }
            }
        } else if let Some(dialogue_text) = talk_objective_dialogue {
            let speaker = self
                .entity_registry
                .get(entity_type)
                .map(|prototype| prototype.display_name.clone())
                .unwrap_or_else(|| entity_type.to_string());
            self.send_to_player(
                player_id,
                ServerMessage::ShowDialogue {
                    quest_id: String::new(),
                    npc_id: npc_id.to_string(),
                    speaker,
                    text: dialogue_text,
                    choices: vec![],
                },
            )
            .await;
        }
    }

    pub(in crate::game) async fn handle_quest_dialogue_choice(
        &self,
        player_id: &str,
        quest_id: &str,
        choice_id: &str,
    ) {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let result = self
            .quest_runner
            .run_on_interact(player_id, quest_id, quest_state, Some(choice_id), None)
            .await;

        let npc_id = self
            .quest_registry
            .get(quest_id)
            .await
            .map(|quest| quest.giver_npc.clone())
            .unwrap_or_default();

        match result {
            Ok(script_result) => {
                self.send_script_dialogue(
                    player_id,
                    quest_id,
                    &npc_id,
                    quest_state,
                    &script_result,
                    true,
                )
                .await;

                if script_result.quest_accepted {
                    self.accept_quest_with_snapshot_progress(player_id, quest_id, quest_state)
                        .await;
                }

                if script_result.quest_completed {
                    self.complete_quest_and_grant_rewards(player_id, quest_id, quest_state)
                        .await;
                }

                self.grant_script_items(player_id, &script_result.granted_items, quest_state)
                    .await;
            }
            Err(error) => {
                tracing::error!("Quest script error: {}", error);
            }
        }
    }

    pub(in crate::game) async fn record_monster_kill(&self, player_id: &str) {
        let Some(db) = self.db.as_ref() else {
            return;
        };

        let Some(character_id) = player_id
            .strip_prefix("char_")
            .and_then(|s| s.parse::<i64>().ok())
        else {
            return;
        };

        if let Err(error) = db.increment_character_monster_kills(character_id, 1).await {
            tracing::warn!(
                "Failed to persist monster kill for {}: {}",
                player_id,
                error
            );
        }
    }

    pub(in crate::game) async fn process_quest_kill(&self, player_id: &str, entity_type: &str) {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::MonsterKilled {
            player_id: player_id.to_string(),
            entity_type: entity_type.to_string(),
            level: 1,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} progress on objective {} for quest {}: {}/{}",
                    player_id,
                    objective_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;

                if result.objective_completed {
                    tracing::info!(
                        "Player {} completed objective {} for quest {}",
                        player_id,
                        objective_id,
                        result.quest_id
                    );
                }
            }

            if result.quest_ready {
                tracing::info!(
                    "Player {} quest {} is ready to complete!",
                    player_id,
                    result.quest_id
                );
            }
        }
    }

    pub(in crate::game) async fn process_quest_item_collect(
        &self,
        player_id: &str,
        item_id: &str,
        count: i32,
    ) {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::ItemCollected {
            player_id: player_id.to_string(),
            item_id: item_id.to_string(),
            count,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} collected quest item objective {} for quest {}: {}/{}",
                    player_id,
                    objective_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;
            }
        }
    }

    pub(in crate::game) async fn process_quest_tree_deplete(
        &self,
        player_id: &str,
        tree_type: &str,
        tree_x: i32,
        tree_y: i32,
    ) {
        tracing::info!(
            "Processing tree depletion quest event: player={}, tree_type={}, pos=({}, {})",
            player_id,
            tree_type,
            tree_x,
            tree_y
        );

        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::TreeDepleted {
            player_id: player_id.to_string(),
            tree_type: tree_type.to_string(),
            x: tree_x,
            y: tree_y,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;
        tracing::info!(
            "Tree depletion quest event returned {} results",
            results.len()
        );

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} depleted tree for quest objective {} in quest {}: {}/{}",
                    player_id,
                    objective_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;
            }
        }
    }

    pub(in crate::game) async fn process_quest_rock_deplete(
        &self,
        player_id: &str,
        rock_type: &str,
        rock_x: i32,
        rock_y: i32,
    ) {
        tracing::info!(
            "Processing rock depletion quest event: player={}, rock_type={}, pos=({}, {})",
            player_id,
            rock_type,
            rock_x,
            rock_y
        );

        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::RockDepleted {
            player_id: player_id.to_string(),
            rock_type: rock_type.to_string(),
            x: rock_x,
            y: rock_y,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;
        tracing::info!(
            "Rock depletion quest event returned {} results",
            results.len()
        );

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} depleted rock for quest objective {} in quest {}: {}/{}",
                    player_id,
                    objective_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;
            }
        }
    }

    pub(in crate::game) async fn process_quest_location_reached(
        &self,
        player_id: &str,
        location_id: &str,
        x: i32,
        y: i32,
    ) {
        let mut quest_states = self.player_quest_states.write().await;
        let quest_state = quest_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerQuestState::new);

        let event = QuestEvent::LocationReached {
            player_id: player_id.to_string(),
            location_id: location_id.to_string(),
            x,
            y,
        };

        let results = self.quest_registry.process_event(&event, quest_state).await;

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} reached location {} for quest {}: {}/{}",
                    player_id,
                    location_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;

                if result.objective_completed {
                    if let Some(quest) = self.quest_registry.get(&result.quest_id).await {
                        if let Some(objective) = quest
                            .objectives
                            .iter()
                            .find(|objective| objective.id == *objective_id)
                        {
                            if let Some(ref dialogue) = objective.dialogue {
                                self.send_to_player(
                                    player_id,
                                    ServerMessage::ShowDialogue {
                                        quest_id: String::new(),
                                        npc_id: String::new(),
                                        speaker: String::new(),
                                        text: dialogue.clone(),
                                        choices: vec![],
                                    },
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn process_quest_progression_snapshot(&self, player_id: &str) {
        let player_snapshot = {
            let players = self.players.read().await;
            players.get(player_id).map(player_progression_snapshot)
        };

        let (gold, skill_levels) = match player_snapshot {
            Some(snapshot) => snapshot,
            None => return,
        };

        let results = {
            let mut quest_states = self.player_quest_states.write().await;
            let quest_state = quest_states
                .entry(player_id.to_string())
                .or_insert_with(PlayerQuestState::new);

            self.quest_registry
                .reconcile_active_quests(quest_state)
                .await;

            let mut results = Vec::new();
            for (skill, level) in &skill_levels {
                let event = QuestEvent::SkillLevelChanged {
                    player_id: player_id.to_string(),
                    skill: (*skill).to_string(),
                    level: *level,
                };
                results.extend(self.quest_registry.process_event(&event, quest_state).await);
            }

            let gold_event = QuestEvent::GoldAmountChanged {
                player_id: player_id.to_string(),
                amount: gold,
            };
            results.extend(
                self.quest_registry
                    .process_event(&gold_event, quest_state)
                    .await,
            );
            results
        };

        for result in results {
            if let (Some(objective_id), Some(current), Some(target)) =
                (&result.objective_id, result.new_progress, result.target)
            {
                tracing::info!(
                    "Player {} progression objective {} for quest {}: {}/{}",
                    player_id,
                    objective_id,
                    result.quest_id,
                    current,
                    target
                );

                self.send_quest_progress_update(
                    player_id,
                    &result.quest_id,
                    objective_id,
                    current,
                    target,
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ServerMessage;
    use crate::quest::{Objective, QuestChain, QuestDialogue, Reward};
    use crate::skills::Skills;
    use std::collections::HashMap;

    #[test]
    fn quest_progress_message_builds_expected_payload() {
        let message = quest_progress_message("quest_a", "objective_b", 2, 5);
        match message {
            ServerMessage::QuestObjectiveProgress {
                quest_id,
                objective_id,
                current,
                target,
            } => {
                assert_eq!(quest_id, "quest_a");
                assert_eq!(objective_id, "objective_b");
                assert_eq!(current, 2);
                assert_eq!(target, 5);
            }
            other => panic!("expected QuestObjectiveProgress, got {:?}", other),
        }
    }

    #[test]
    fn player_progression_snapshot_includes_expected_skill_entries() {
        let mut player =
            super::super::Player::new("char_1", "Tester", 0, 0, "neutral", "light", None, None);
        player.inventory.gold = 250;
        player.skills = Skills::new();

        let (gold, entries) = player_progression_snapshot(&player);

        assert_eq!(gold, 250);
        assert!(entries.contains(&("attack", player.skills.attack.level)));
        assert!(entries.contains(&("strength", player.skills.strength.level)));
        assert!(entries.contains(&("defence", player.skills.defence.level)));
        assert!(entries.contains(&("fishing", player.skills.fishing.level)));
        assert!(entries.contains(&("alchemy", player.skills.alchemy.level)));
        assert!(entries.contains(&("combat", player.skills.combat_level())));
        assert_eq!(entries.len(), 13);
    }

    #[test]
    fn greeting_dialogue_metadata_uses_tutorial_choices_for_old_thomas() {
        let (quest_id, choices) = greeting_dialogue_metadata("old_thomas");

        assert_eq!(quest_id, "__tutorial__");
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].id, "accept");
        assert_eq!(choices[1].id, "skip");
    }

    #[test]
    fn select_target_quest_restarts_repeatable_quests_before_completed_fallbacks() {
        let repeatable = Quest {
            id: "repeatable".to_string(),
            name: "Repeatable".to_string(),
            description: String::new(),
            giver_npc: "npc".to_string(),
            level_required: 1,
            lua_script: None,
            chain: QuestChain::default(),
            objectives: vec![Objective {
                id: "objective".to_string(),
                objective_type: ObjectiveType::TalkTo,
                target: "npc".to_string(),
                count: 1,
                description: String::new(),
                sequential: false,
                dialogue: None,
                consume: true,
                aliases: vec![],
            }],
            rewards: Reward::default(),
            dialogue: QuestDialogue::default(),
            repeatable: true,
        };
        let completed = Quest {
            id: "completed".to_string(),
            name: "Completed".to_string(),
            description: String::new(),
            giver_npc: "npc".to_string(),
            level_required: 1,
            lua_script: None,
            chain: QuestChain::default(),
            objectives: repeatable.objectives.clone(),
            rewards: Reward::default(),
            dialogue: QuestDialogue::default(),
            repeatable: false,
        };
        let mut quest_state = PlayerQuestState {
            active_quests: HashMap::new(),
            completed_quests: vec!["repeatable".to_string(), "completed".to_string()],
            available_quests: Vec::new(),
            flags: HashMap::new(),
        };

        let selected = select_target_quest(
            &[Arc::new(completed), Arc::new(repeatable)],
            &mut quest_state,
        );

        assert_eq!(selected, Some(("repeatable".to_string(), "not_started")));
        assert!(!quest_state.is_quest_completed("repeatable"));
        assert!(quest_state.is_quest_completed("completed"));
    }
}
