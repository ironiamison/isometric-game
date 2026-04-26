use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceContractKind {
    Farming,
    Mining,
    Woodcutting,
    Fishing,
    Smithing,
}

impl ResourceContractKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceContractKind::Farming => "farming",
            ResourceContractKind::Mining => "mining",
            ResourceContractKind::Woodcutting => "woodcutting",
            ResourceContractKind::Fishing => "fishing",
            ResourceContractKind::Smithing => "smithing",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "farming" => Some(ResourceContractKind::Farming),
            "mining" => Some(ResourceContractKind::Mining),
            "woodcutting" => Some(ResourceContractKind::Woodcutting),
            "fishing" => Some(ResourceContractKind::Fishing),
            "smithing" => Some(ResourceContractKind::Smithing),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ResourceContractKind::Farming => "Farming",
            ResourceContractKind::Mining => "Mining",
            ResourceContractKind::Woodcutting => "Woodcutting",
            ResourceContractKind::Fishing => "Fishing",
            ResourceContractKind::Smithing => "Smithing",
        }
    }

    pub fn action_text(&self) -> &'static str {
        match self {
            ResourceContractKind::Farming => "Harvest",
            ResourceContractKind::Mining => "Mine",
            ResourceContractKind::Woodcutting => "Chop",
            ResourceContractKind::Fishing => "Catch",
            ResourceContractKind::Smithing => "Smith",
        }
    }

    pub fn progress_label(&self) -> &'static str {
        match self {
            ResourceContractKind::Farming => "harvested",
            ResourceContractKind::Mining => "mined",
            ResourceContractKind::Woodcutting => "chopped",
            ResourceContractKind::Fishing => "caught",
            ResourceContractKind::Smithing => "crafted",
        }
    }

    pub fn skill_name(&self) -> &'static str {
        match self {
            ResourceContractKind::Farming => "farming",
            ResourceContractKind::Mining => "mining",
            ResourceContractKind::Woodcutting => "woodcutting",
            ResourceContractKind::Fishing => "fishing",
            ResourceContractKind::Smithing => "smithing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractDifficulty {
    Easy,
    Medium,
    Hard,
}

impl ContractDifficulty {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContractDifficulty::Easy => "easy",
            ContractDifficulty::Medium => "medium",
            ContractDifficulty::Hard => "hard",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "easy" => Some(ContractDifficulty::Easy),
            "medium" => Some(ContractDifficulty::Medium),
            "hard" => Some(ContractDifficulty::Hard),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ContractDifficulty::Easy => "Easy",
            ContractDifficulty::Medium => "Medium",
            ContractDifficulty::Hard => "Hard",
        }
    }

    pub fn level_required(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 15,
            ContractDifficulty::Hard => 30,
        }
    }

    pub fn minimum_target_level(&self) -> i32 {
        self.level_required()
    }

    pub fn target_amount_range(&self, kind: ResourceContractKind) -> (i32, i32) {
        match (kind, self) {
            (ResourceContractKind::Farming, ContractDifficulty::Easy) => (3, 5),
            (ResourceContractKind::Farming, ContractDifficulty::Medium) => (6, 10),
            (ResourceContractKind::Farming, ContractDifficulty::Hard) => (12, 18),
            (ResourceContractKind::Fishing, ContractDifficulty::Easy) => (4, 7),
            (ResourceContractKind::Fishing, ContractDifficulty::Medium) => (8, 12),
            (ResourceContractKind::Fishing, ContractDifficulty::Hard) => (12, 18),
            (ResourceContractKind::Smithing, ContractDifficulty::Easy) => (2, 3),
            (ResourceContractKind::Smithing, ContractDifficulty::Medium) => (4, 6),
            (ResourceContractKind::Smithing, ContractDifficulty::Hard) => (6, 8),
            (_, ContractDifficulty::Easy) => (6, 10),
            (_, ContractDifficulty::Medium) => (10, 16),
            (_, ContractDifficulty::Hard) => (16, 24),
        }
    }

    pub fn xp_reward(&self, kind: ResourceContractKind) -> i64 {
        match (kind, self) {
            (ResourceContractKind::Farming, ContractDifficulty::Easy) => 150,
            (ResourceContractKind::Farming, ContractDifficulty::Medium) => 500,
            (ResourceContractKind::Farming, ContractDifficulty::Hard) => 1200,
            (ResourceContractKind::Fishing, ContractDifficulty::Easy) => 170,
            (ResourceContractKind::Fishing, ContractDifficulty::Medium) => 540,
            (ResourceContractKind::Fishing, ContractDifficulty::Hard) => 1280,
            (ResourceContractKind::Smithing, ContractDifficulty::Easy) => 220,
            (ResourceContractKind::Smithing, ContractDifficulty::Medium) => 700,
            (ResourceContractKind::Smithing, ContractDifficulty::Hard) => 1600,
            (_, ContractDifficulty::Easy) => 180,
            (_, ContractDifficulty::Medium) => 575,
            (_, ContractDifficulty::Hard) => 1350,
        }
    }

    pub fn gold_reward(&self, kind: ResourceContractKind) -> i32 {
        match (kind, self) {
            (ResourceContractKind::Farming, ContractDifficulty::Easy) => 100,
            (ResourceContractKind::Farming, ContractDifficulty::Medium) => 350,
            (ResourceContractKind::Farming, ContractDifficulty::Hard) => 800,
            (ResourceContractKind::Fishing, ContractDifficulty::Easy) => 110,
            (ResourceContractKind::Fishing, ContractDifficulty::Medium) => 360,
            (ResourceContractKind::Fishing, ContractDifficulty::Hard) => 820,
            (ResourceContractKind::Smithing, ContractDifficulty::Easy) => 150,
            (ResourceContractKind::Smithing, ContractDifficulty::Medium) => 475,
            (ResourceContractKind::Smithing, ContractDifficulty::Hard) => 1050,
            (_, ContractDifficulty::Easy) => 120,
            (_, ContractDifficulty::Medium) => 400,
            (_, ContractDifficulty::Hard) => 900,
        }
    }

    pub fn farming_seed_reward_count(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 2,
            ContractDifficulty::Hard => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceContract {
    pub player_id: String,
    pub kind: ResourceContractKind,
    pub difficulty: ContractDifficulty,
    pub target_item_id: String,
    pub target_name: String,
    pub amount_required: i32,
    pub amount_completed: i32,
    pub created_at: u64,
    pub giver_npc_id: String,
    pub giver_name: String,
}

impl ResourceContract {
    pub fn is_complete(&self) -> bool {
        self.amount_completed >= self.amount_required
    }

    pub fn task_text(&self) -> String {
        format!("{} {}", self.kind.action_text(), self.target_name)
    }
}

pub struct ResourceContractManager {
    contracts: HashMap<String, ResourceContract>,
}

impl ResourceContractManager {
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
        }
    }

    pub fn has_contract(&self, player_id: &str) -> bool {
        self.contracts.contains_key(player_id)
    }

    pub fn get_contract(&self, player_id: &str) -> Option<&ResourceContract> {
        self.contracts.get(player_id)
    }

    pub fn insert_contract(&mut self, contract: ResourceContract) {
        self.contracts.insert(contract.player_id.clone(), contract);
    }

    pub fn remove_contract(&mut self, player_id: &str) -> Option<ResourceContract> {
        self.contracts.remove(player_id)
    }

    pub fn record_item_progress(
        &mut self,
        player_id: &str,
        item_id: &str,
        amount: i32,
    ) -> Option<(i32, i32, bool)> {
        let contract = self.contracts.get_mut(player_id)?;
        if contract.target_item_id != item_id || contract.is_complete() {
            return None;
        }

        contract.amount_completed = (contract.amount_completed + amount).min(contract.amount_required);
        Some((
            contract.amount_completed,
            contract.amount_required,
            contract.is_complete(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore_contract(
        &mut self,
        player_id: &str,
        kind: &str,
        difficulty: &str,
        target_item_id: &str,
        target_name: &str,
        amount_required: i32,
        amount_completed: i32,
        giver_npc_id: &str,
        giver_name: &str,
        created_at: u64,
    ) {
        let Some(kind) = ResourceContractKind::from_str(kind) else {
            return;
        };
        let Some(difficulty) = ContractDifficulty::from_str(difficulty) else {
            return;
        };

        self.insert_contract(ResourceContract {
            player_id: player_id.to_string(),
            kind,
            difficulty,
            target_item_id: target_item_id.to_string(),
            target_name: target_name.to_string(),
            amount_required,
            amount_completed,
            created_at,
            giver_npc_id: giver_npc_id.to_string(),
            giver_name: giver_name.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_item_progress_caps_at_requirement() {
        let mut manager = ResourceContractManager::new();
        manager.insert_contract(ResourceContract {
            player_id: "p1".to_string(),
            kind: ResourceContractKind::Mining,
            difficulty: ContractDifficulty::Easy,
            target_item_id: "copper_ore".to_string(),
            target_name: "Copper Ore".to_string(),
            amount_required: 6,
            amount_completed: 0,
            created_at: 1,
            giver_npc_id: "miner_mike".to_string(),
            giver_name: "Miner Mike".to_string(),
        });

        assert_eq!(
            manager.record_item_progress("p1", "copper_ore", 10),
            Some((6, 6, true))
        );
        assert_eq!(manager.record_item_progress("p1", "copper_ore", 1), None);
    }

    #[test]
    fn restore_contract_ignores_unknown_values() {
        let mut manager = ResourceContractManager::new();
        manager.restore_contract(
            "p1",
            "unknown",
            "easy",
            "oak_log",
            "Oak Log",
            5,
            0,
            "lumberjack_pete",
            "Lumberjack Pete",
            1,
        );
        assert!(!manager.has_contract("p1"));
    }
}
