use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::QuestAccepted {
            quest_id,
            quest_name,
            objectives,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("quest_name".into()),
                Value::String(quest_name.clone().into()),
            ));

            let obj_values: Vec<Value> = objectives
                .iter()
                .map(|obj| {
                    let mut omap = Vec::new();
                    omap.push((
                        Value::String("id".into()),
                        Value::String(obj.id.clone().into()),
                    ));
                    omap.push((
                        Value::String("description".into()),
                        Value::String(obj.description.clone().into()),
                    ));
                    omap.push((
                        Value::String("current".into()),
                        Value::Integer((obj.current as i64).into()),
                    ));
                    omap.push((
                        Value::String("target".into()),
                        Value::Integer((obj.target as i64).into()),
                    ));
                    omap.push((
                        Value::String("completed".into()),
                        Value::Boolean(obj.completed),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("objectives".into()), Value::Array(obj_values)));

            Value::Map(map)
        }
        ServerMessage::QuestObjectiveProgress {
            quest_id,
            objective_id,
            current,
            target,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("objective_id".into()),
                Value::String(objective_id.clone().into()),
            ));
            map.push((
                Value::String("current".into()),
                Value::Integer((*current as i64).into()),
            ));
            map.push((
                Value::String("target".into()),
                Value::Integer((*target as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestCompleted {
            quest_id,
            quest_name,
            rewards_exp,
            rewards_gold,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("quest_name".into()),
                Value::String(quest_name.clone().into()),
            ));
            map.push((
                Value::String("rewards_exp".into()),
                Value::Integer((*rewards_exp as i64).into()),
            ));
            map.push((
                Value::String("rewards_gold".into()),
                Value::Integer((*rewards_gold as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestStateSync {
            completed_quest_ids,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("completed_quest_ids".into()),
                Value::Array(
                    completed_quest_ids
                        .iter()
                        .map(|id| Value::String(id.clone().into()))
                        .collect(),
                ),
            ));
            Value::Map(map)
        }
        ServerMessage::QuestCatalog { quests } => {
            let mut map = Vec::new();
            let quest_values: Vec<Value> = quests
                .iter()
                .map(|q| {
                    let mut qmap = Vec::new();
                    qmap.push((
                        Value::String("quest_id".into()),
                        Value::String(q.quest_id.clone().into()),
                    ));
                    qmap.push((
                        Value::String("name".into()),
                        Value::String(q.name.clone().into()),
                    ));
                    qmap.push((
                        Value::String("description".into()),
                        Value::String(q.description.clone().into()),
                    ));
                    qmap.push((
                        Value::String("giver_npc_name".into()),
                        Value::String(q.giver_npc_name.clone().into()),
                    ));
                    qmap.push((
                        Value::String("level_required".into()),
                        Value::Integer((q.level_required as i64).into()),
                    ));
                    if let Some(ref req_id) = q.required_quest_id {
                        qmap.push((
                            Value::String("required_quest_id".into()),
                            Value::String(req_id.clone().into()),
                        ));
                    }
                    if let Some(ref req_name) = q.required_quest_name {
                        qmap.push((
                            Value::String("required_quest_name".into()),
                            Value::String(req_name.clone().into()),
                        ));
                    }
                    let obj_values: Vec<Value> = q
                        .objectives
                        .iter()
                        .map(|obj| {
                            let mut omap = Vec::new();
                            omap.push((
                                Value::String("id".into()),
                                Value::String(obj.id.clone().into()),
                            ));
                            omap.push((
                                Value::String("description".into()),
                                Value::String(obj.description.clone().into()),
                            ));
                            omap.push((
                                Value::String("target".into()),
                                Value::Integer((obj.target as i64).into()),
                            ));
                            Value::Map(omap)
                        })
                        .collect();
                    qmap.push((Value::String("objectives".into()), Value::Array(obj_values)));
                    Value::Map(qmap)
                })
                .collect();
            map.push((Value::String("quests".into()), Value::Array(quest_values)));
            Value::Map(map)
        }
        ServerMessage::ShowDialogue {
            quest_id,
            npc_id,
            speaker,
            text,
            choices,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("quest_id".into()),
                Value::String(quest_id.clone().into()),
            ));
            map.push((
                Value::String("npc_id".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("speaker".into()),
                Value::String(speaker.clone().into()),
            ));
            map.push((
                Value::String("text".into()),
                Value::String(text.clone().into()),
            ));

            let choice_values: Vec<Value> = choices
                .iter()
                .map(|c| {
                    let mut cmap = Vec::new();
                    cmap.push((
                        Value::String("id".into()),
                        Value::String(c.id.clone().into()),
                    ));
                    cmap.push((
                        Value::String("text".into()),
                        Value::String(c.text.clone().into()),
                    ));
                    Value::Map(cmap)
                })
                .collect();
            map.push((Value::String("choices".into()), Value::Array(choice_values)));

            Value::Map(map)
        }
        ServerMessage::DialogueClosed => {
            // Empty map - just the message type signals closure
            Value::Map(Vec::new())
        }
        ServerMessage::FarmingContractUpdate {
            active,
            difficulty,
            crop_name,
            amount_required,
            amount_harvested,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("active".into()), Value::Boolean(*active)));
            map.push((
                Value::String("difficulty".into()),
                Value::String(difficulty.clone().into()),
            ));
            map.push((
                Value::String("crop_name".into()),
                Value::String(crop_name.clone().into()),
            ));
            map.push((
                Value::String("amount_required".into()),
                Value::Integer((*amount_required as i64).into()),
            ));
            map.push((
                Value::String("amount_harvested".into()),
                Value::Integer((*amount_harvested as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ResourceContractUpdate {
            active,
            contract_kind,
            difficulty,
            task_text,
            progress_label,
            amount_required,
            amount_completed,
            giver_name,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("active".into()), Value::Boolean(*active)));
            map.push((
                Value::String("contract_kind".into()),
                Value::String(contract_kind.clone().into()),
            ));
            map.push((
                Value::String("difficulty".into()),
                Value::String(difficulty.clone().into()),
            ));
            map.push((
                Value::String("task_text".into()),
                Value::String(task_text.clone().into()),
            ));
            map.push((
                Value::String("progress_label".into()),
                Value::String(progress_label.clone().into()),
            ));
            map.push((
                Value::String("amount_required".into()),
                Value::Integer((*amount_required as i64).into()),
            ));
            map.push((
                Value::String("amount_completed".into()),
                Value::Integer((*amount_completed as i64).into()),
            ));
            map.push((
                Value::String("giver_name".into()),
                Value::String(giver_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::AdventureBoardState {
            npc_id,
            offers,
            active_contract,
            stats,
            crafting_orders,
            crafting_order_active,
            crafting_order_stats,
            daily_contracts_completed,
            daily_contract_limit,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npc_id".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("offers".into()),
                Value::Array(
                    offers
                        .iter()
                        .map(|offer| {
                            let mut offer_map = Vec::new();
                            offer_map.push((
                                Value::String("kind_id".into()),
                                Value::String(offer.kind_id.clone().into()),
                            ));
                            offer_map.push((
                                Value::String("kind_name".into()),
                                Value::String(offer.kind_name.clone().into()),
                            ));
                            offer_map.push((
                                Value::String("description".into()),
                                Value::String(offer.description.clone().into()),
                            ));
                            offer_map.push((
                                Value::String("skill_level".into()),
                                Value::Integer((offer.skill_level as i64).into()),
                            ));
                            offer_map.push((
                                Value::String("difficulties".into()),
                                Value::Array(
                                    offer
                                        .difficulties
                                        .iter()
                                        .map(|difficulty| {
                                            let mut diff_map = Vec::new();
                                            diff_map.push((
                                                Value::String("difficulty_id".into()),
                                                Value::String(
                                                    difficulty.difficulty_id.clone().into(),
                                                ),
                                            ));
                                            diff_map.push((
                                                Value::String("difficulty_name".into()),
                                                Value::String(
                                                    difficulty.difficulty_name.clone().into(),
                                                ),
                                            ));
                                            diff_map.push((
                                                Value::String("level_required".into()),
                                                Value::Integer(
                                                    (difficulty.level_required as i64).into(),
                                                ),
                                            ));
                                            diff_map.push((
                                                Value::String("unlocked".into()),
                                                Value::Boolean(difficulty.unlocked),
                                            ));
                                            diff_map.push((
                                                Value::String("reward_xp".into()),
                                                Value::Integer(difficulty.reward_xp.into()),
                                            ));
                                            diff_map.push((
                                                Value::String("reward_gold".into()),
                                                Value::Integer(
                                                    (difficulty.reward_gold as i64).into(),
                                                ),
                                            ));
                                            Value::Map(diff_map)
                                        })
                                        .collect(),
                                ),
                            ));
                            Value::Map(offer_map)
                        })
                        .collect(),
                ),
            ));
            let active_value = if let Some(contract) = active_contract {
                let mut contract_map = Vec::new();
                contract_map.push((
                    Value::String("kind_id".into()),
                    Value::String(contract.kind_id.clone().into()),
                ));
                contract_map.push((
                    Value::String("kind_name".into()),
                    Value::String(contract.kind_name.clone().into()),
                ));
                contract_map.push((
                    Value::String("difficulty_name".into()),
                    Value::String(contract.difficulty_name.clone().into()),
                ));
                contract_map.push((
                    Value::String("task_text".into()),
                    Value::String(contract.task_text.clone().into()),
                ));
                contract_map.push((
                    Value::String("progress_label".into()),
                    Value::String(contract.progress_label.clone().into()),
                ));
                contract_map.push((
                    Value::String("amount_required".into()),
                    Value::Integer((contract.amount_required as i64).into()),
                ));
                contract_map.push((
                    Value::String("amount_completed".into()),
                    Value::Integer((contract.amount_completed as i64).into()),
                ));
                contract_map.push((
                    Value::String("giver_name".into()),
                    Value::String(contract.giver_name.clone().into()),
                ));
                contract_map.push((
                    Value::String("reward_xp".into()),
                    Value::Integer(contract.reward_xp.into()),
                ));
                contract_map.push((
                    Value::String("reward_gold".into()),
                    Value::Integer((contract.reward_gold as i64).into()),
                ));
                contract_map.push((
                    Value::String("bonus_item_text".into()),
                    Value::String(contract.bonus_item_text.clone().into()),
                ));
                contract_map.push((
                    Value::String("can_claim".into()),
                    Value::Boolean(contract.can_claim),
                ));
                Value::Map(contract_map)
            } else {
                Value::Nil
            };
            map.push((Value::String("active_contract".into()), active_value));
            let mut stats_map = Vec::new();
            stats_map.push((
                Value::String("contracts_completed".into()),
                Value::Integer((stats.contracts_completed as i64).into()),
            ));
            stats_map.push((
                Value::String("total_gold_earned".into()),
                Value::Integer((stats.total_gold_earned as i64).into()),
            ));
            stats_map.push((
                Value::String("total_xp_earned".into()),
                Value::Integer(stats.total_xp_earned.into()),
            ));
            map.push((Value::String("stats".into()), Value::Map(stats_map)));

            // Crafting orders tab
            map.push((
                Value::String("crafting_orders".into()),
                Value::Array(
                    crafting_orders
                        .iter()
                        .map(|order| {
                            let mut order_map = Vec::new();
                            order_map.push((
                                Value::String("order_id".into()),
                                Value::String(order.order_id.clone().into()),
                            ));
                            order_map.push((
                                Value::String("tier".into()),
                                Value::String(order.tier.clone().into()),
                            ));
                            order_map.push((
                                Value::String("skill".into()),
                                Value::String(order.skill.clone().into()),
                            ));
                            order_map.push((
                                Value::String("min_level".into()),
                                Value::Integer((order.min_level as i64).into()),
                            ));
                            order_map.push((
                                Value::String("items".into()),
                                Value::Array(
                                    order
                                        .items
                                        .iter()
                                        .map(|item| {
                                            let mut item_map = Vec::new();
                                            item_map.push((
                                                Value::String("item_id".into()),
                                                Value::String(item.item_id.clone().into()),
                                            ));
                                            item_map.push((
                                                Value::String("item_name".into()),
                                                Value::String(item.item_name.clone().into()),
                                            ));
                                            item_map.push((
                                                Value::String("quantity".into()),
                                                Value::Integer((item.quantity as i64).into()),
                                            ));
                                            Value::Map(item_map)
                                        })
                                        .collect(),
                                ),
                            ));
                            order_map.push((
                                Value::String("reward_gold".into()),
                                Value::Integer((order.reward_gold as i64).into()),
                            ));
                            order_map.push((
                                Value::String("reward_xp".into()),
                                Value::Array(
                                    order
                                        .reward_xp
                                        .iter()
                                        .map(|(skill, amount)| {
                                            let mut xp_map = Vec::new();
                                            xp_map.push((
                                                Value::String("skill".into()),
                                                Value::String(skill.clone().into()),
                                            ));
                                            xp_map.push((
                                                Value::String("amount".into()),
                                                Value::Integer((*amount).into()),
                                            ));
                                            Value::Map(xp_map)
                                        })
                                        .collect(),
                                ),
                            ));
                            order_map.push((
                                Value::String("reward_marks".into()),
                                Value::Integer((order.reward_marks as i64).into()),
                            ));
                            Value::Map(order_map)
                        })
                        .collect(),
                ),
            ));

            let crafting_active_value = if let Some(active) = crafting_order_active {
                let mut active_map = Vec::new();
                active_map.push((
                    Value::String("order_id".into()),
                    Value::String(active.order_id.clone().into()),
                ));
                active_map.push((
                    Value::String("tier".into()),
                    Value::String(active.tier.clone().into()),
                ));
                active_map.push((
                    Value::String("skill".into()),
                    Value::String(active.skill.clone().into()),
                ));
                active_map.push((
                    Value::String("items".into()),
                    Value::Array(
                        active
                            .items
                            .iter()
                            .map(|item| {
                                let mut item_map = Vec::new();
                                item_map.push((
                                    Value::String("item_id".into()),
                                    Value::String(item.item_id.clone().into()),
                                ));
                                item_map.push((
                                    Value::String("item_name".into()),
                                    Value::String(item.item_name.clone().into()),
                                ));
                                item_map.push((
                                    Value::String("quantity".into()),
                                    Value::Integer((item.quantity as i64).into()),
                                ));
                                Value::Map(item_map)
                            })
                            .collect(),
                    ),
                ));
                active_map.push((
                    Value::String("reward_gold".into()),
                    Value::Integer((active.reward_gold as i64).into()),
                ));
                active_map.push((
                    Value::String("reward_marks".into()),
                    Value::Integer((active.reward_marks as i64).into()),
                ));
                active_map.push((
                    Value::String("can_claim".into()),
                    Value::Boolean(active.can_claim),
                ));
                Value::Map(active_map)
            } else {
                Value::Nil
            };
            map.push((
                Value::String("crafting_order_active".into()),
                crafting_active_value,
            ));

            let mut co_stats_map = Vec::new();
            co_stats_map.push((
                Value::String("orders_completed".into()),
                Value::Integer((crafting_order_stats.orders_completed as i64).into()),
            ));
            co_stats_map.push((
                Value::String("masterwork_completed".into()),
                Value::Integer((crafting_order_stats.masterwork_completed as i64).into()),
            ));
            co_stats_map.push((
                Value::String("commission_marks".into()),
                Value::Integer((crafting_order_stats.commission_marks as i64).into()),
            ));
            map.push((
                Value::String("crafting_order_stats".into()),
                Value::Map(co_stats_map),
            ));
            map.push((
                Value::String("daily_contracts_completed".into()),
                Value::Integer((*daily_contracts_completed as i64).into()),
            ));
            map.push((
                Value::String("daily_contract_limit".into()),
                Value::Integer((*daily_contract_limit as i64).into()),
            ));

            Value::Map(map)
        }
        ServerMessage::SlayerPanelOpen {
            master_id,
            master_name,
            current_task,
            points,
            tasks_completed,
            rewards,
            blocked_monsters,
            unlocked_monsters,
            blockable_monsters,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("master_id".into()),
                Value::String(master_id.clone().into()),
            ));
            map.push((
                Value::String("master_name".into()),
                Value::String(master_name.clone().into()),
            ));
            map.push((
                Value::String("current_task".into()),
                slayer_task_to_value(current_task),
            ));
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            map.push((
                Value::String("tasks_completed".into()),
                Value::Integer((*tasks_completed as i64).into()),
            ));
            let reward_values: Vec<Value> =
                rewards.iter().map(slayer_reward_to_value).collect();
            map.push((Value::String("rewards".into()), Value::Array(reward_values)));
            let blocked: Vec<Value> = blocked_monsters
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((
                Value::String("blocked_monsters".into()),
                Value::Array(blocked),
            ));
            let unlocked: Vec<Value> = unlocked_monsters
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((
                Value::String("unlocked_monsters".into()),
                Value::Array(unlocked),
            ));
            let blockable: Vec<Value> = blockable_monsters
                .iter()
                .map(|(id, name)| {
                    Value::Map(vec![
                        (Value::String("id".into()), Value::String(id.clone().into())),
                        (
                            Value::String("name".into()),
                            Value::String(name.clone().into()),
                        ),
                    ])
                })
                .collect();
            map.push((
                Value::String("blockable_monsters".into()),
                Value::Array(blockable),
            ));
            Value::Map(map)
        }
        ServerMessage::SlayerTaskProgress {
            monster_id,
            display_name,
            kills_current,
            kills_required,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("monster_id".into()),
                Value::String(monster_id.clone().into()),
            ));
            map.push((
                Value::String("display_name".into()),
                Value::String(display_name.clone().into()),
            ));
            map.push((
                Value::String("kills_current".into()),
                Value::Integer((*kills_current as i64).into()),
            ));
            map.push((
                Value::String("kills_required".into()),
                Value::Integer((*kills_required as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SlayerTaskComplete {
            monster_id,
            display_name,
            points_awarded,
            total_points,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("monster_id".into()),
                Value::String(monster_id.clone().into()),
            ));
            map.push((
                Value::String("display_name".into()),
                Value::String(display_name.clone().into()),
            ));
            map.push((
                Value::String("points_awarded".into()),
                Value::Integer((*points_awarded as i64).into()),
            ));
            map.push((
                Value::String("total_points".into()),
                Value::Integer((*total_points as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::SlayerResult {
            success,
            action,
            message,
            task,
            points,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            map.push((Value::String("task".into()), slayer_task_to_value(task)));
            map.push((
                Value::String("points".into()),
                match points {
                    Some(p) => Value::Integer((*p as i64).into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::SlayerStateSync {
            current_task,
            points,
            tasks_completed,
            blocked_monsters,
            unlocked_monsters,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("current_task".into()),
                slayer_task_to_value(current_task),
            ));
            map.push((
                Value::String("points".into()),
                Value::Integer((*points as i64).into()),
            ));
            map.push((
                Value::String("tasks_completed".into()),
                Value::Integer((*tasks_completed as i64).into()),
            ));
            let blocked: Vec<Value> = blocked_monsters
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((
                Value::String("blocked_monsters".into()),
                Value::Array(blocked),
            ));
            let unlocked: Vec<Value> = unlocked_monsters
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            map.push((
                Value::String("unlocked_monsters".into()),
                Value::Array(unlocked),
            ));
            Value::Map(map)
        }
        ServerMessage::AutoActionStarted {
            target_type,
            target_id,
            action,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("target_type".into()),
                Value::String(target_type.clone().into()),
            ));
            map.push((
                Value::String("target_id".into()),
                Value::String(target_id.clone().into()),
            ));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::AutoActionStopped { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
