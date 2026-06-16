use super::*;

mod contracts;
mod patches;

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

fn patch_update_message(update: &crate::farming::PatchUpdate) -> ServerMessage {
    ServerMessage::PatchStateUpdate {
        patch_id: update.patch_id.clone(),
        state: update.state.clone(),
        crop_id: update.crop_id.clone(),
        growth_stage: update.growth_stage,
        owner_id: update.owner_id.clone(),
        health: update.health.clone(),
        lives_remaining: update.lives_remaining,
        composted: update.composted,
        patch_type: update.patch_type.clone(),
    }
}

fn plot_purchase_choice(
    req: &crate::farming::PlotRequirement,
    owned: bool,
    farming_level: i32,
    gold: i32,
) -> crate::protocol::DialogueChoice {
    if owned {
        crate::protocol::DialogueChoice {
            id: format!("owned_{}", req.plot_id),
            text: format!("Plot {} (Owned)", req.plot_id),
        }
    } else if farming_level < req.farming_level {
        crate::protocol::DialogueChoice {
            id: format!("locked_{}", req.plot_id),
            text: format!(
                "Plot {} - {}gp (Requires Farming {})",
                req.plot_id, req.gold_cost, req.farming_level
            ),
        }
    } else if gold < req.gold_cost {
        crate::protocol::DialogueChoice {
            id: format!("locked_{}", req.plot_id),
            text: format!(
                "Plot {} - {}gp (Not enough gold)",
                req.plot_id, req.gold_cost
            ),
        }
    } else {
        crate::protocol::DialogueChoice {
            id: format!("unlock_{}", req.plot_id),
            text: format!("Plot {} - {}gp", req.plot_id, req.gold_cost),
        }
    }
}

fn contract_choice(
    difficulty: &crate::farming::ContractDifficulty,
    farming_level: i32,
) -> crate::protocol::DialogueChoice {
    if farming_level >= difficulty.level_required() {
        crate::protocol::DialogueChoice {
            id: format!("accept_{}", difficulty.as_str()),
            text: format!(
                "{} - {}xp, {}gp",
                difficulty.display_name(),
                difficulty.xp_reward(),
                difficulty.gold_reward()
            ),
        }
    } else {
        crate::protocol::DialogueChoice {
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

fn farming_growth_is_due(current_tick: u64) -> bool {
    current_tick % 100 == 50
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
}
