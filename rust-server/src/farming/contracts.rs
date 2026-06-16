use crate::farming::{CropConfig, FarmingSystem};
use rand::Rng;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "easy" => Some(ContractDifficulty::Easy),
            "medium" => Some(ContractDifficulty::Medium),
            "hard" => Some(ContractDifficulty::Hard),
            _ => None,
        }
    }

    pub fn level_required(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 15,
            ContractDifficulty::Hard => 30,
        }
    }

    pub fn harvest_range(&self) -> (i32, i32) {
        match self {
            ContractDifficulty::Easy => (3, 5),
            ContractDifficulty::Medium => (6, 10),
            ContractDifficulty::Hard => (12, 18),
        }
    }

    pub fn xp_reward(&self) -> i64 {
        match self {
            ContractDifficulty::Easy => 150,
            ContractDifficulty::Medium => 500,
            ContractDifficulty::Hard => 1200,
        }
    }

    pub fn gold_reward(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 100,
            ContractDifficulty::Medium => 350,
            ContractDifficulty::Hard => 800,
        }
    }

    pub fn seed_reward_count(&self) -> i32 {
        match self {
            ContractDifficulty::Easy => 1,
            ContractDifficulty::Medium => 2,
            ContractDifficulty::Hard => 3,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ContractDifficulty::Easy => "Easy",
            ContractDifficulty::Medium => "Medium",
            ContractDifficulty::Hard => "Hard",
        }
    }
}

/// An active farming contract for a player
#[derive(Debug, Clone)]
pub struct FarmingContract {
    pub player_id: String,
    pub difficulty: ContractDifficulty,
    pub crop_id: String,
    pub amount_required: i32,
    pub amount_harvested: i32,
    pub created_at: u64,
}

impl FarmingContract {
    pub fn is_complete(&self) -> bool {
        self.amount_harvested >= self.amount_required
    }

    pub fn produce_item(&self, crops: &HashMap<String, CropConfig>) -> String {
        crops
            .get(&self.crop_id)
            .map(|c| c.produce_item.clone())
            .unwrap_or_else(|| self.crop_id.clone())
    }
}

impl FarmingSystem {
    /// Generate a new farming contract for a player
    pub fn generate_contract(
        &mut self,
        player_id: &str,
        difficulty: &ContractDifficulty,
        farming_level: i32,
        current_time: u64,
    ) -> Result<&FarmingContract, String> {
        if self.contracts.contains_key(player_id) {
            return Err("You already have an active contract".to_string());
        }

        if farming_level < difficulty.level_required() {
            return Err(format!(
                "You need Farming level {} for {} contracts",
                difficulty.level_required(),
                difficulty.display_name()
            ));
        }

        let eligible_crops: Vec<(&str, &CropConfig)> = self
            .crops
            .iter()
            .filter(|(_, c)| c.level_required <= farming_level)
            .map(|(id, c)| (id.as_str(), c))
            .collect();

        if eligible_crops.is_empty() {
            return Err("No crops available for your level".to_string());
        }

        let mut rng = rand::thread_rng();
        let (crop_id, _) = eligible_crops[rng.gen_range(0..eligible_crops.len())];
        let (min, max) = difficulty.harvest_range();
        let amount = rng.gen_range(min..=max);

        let contract = FarmingContract {
            player_id: player_id.to_string(),
            difficulty: difficulty.clone(),
            crop_id: crop_id.to_string(),
            amount_required: amount,
            amount_harvested: 0,
            created_at: current_time,
        };

        self.contracts.insert(player_id.to_string(), contract);
        Ok(self.contracts.get(player_id).unwrap())
    }

    /// Record a harvest toward a player's active contract
    pub fn record_contract_harvest(
        &mut self,
        player_id: &str,
        crop_id: &str,
        amount: i32,
    ) -> Option<(i32, i32, bool)> {
        let contract = self.contracts.get_mut(player_id)?;
        if contract.crop_id != crop_id || contract.is_complete() {
            return None;
        }
        contract.amount_harvested =
            (contract.amount_harvested + amount).min(contract.amount_required);
        let complete = contract.is_complete();
        Some((
            contract.amount_harvested,
            contract.amount_required,
            complete,
        ))
    }

    /// Get a player's active contract
    pub fn get_contract(&self, player_id: &str) -> Option<&FarmingContract> {
        self.contracts.get(player_id)
    }

    /// Remove a player's contract (on completion or abandonment)
    pub fn remove_contract(&mut self, player_id: &str) -> Option<FarmingContract> {
        self.contracts.remove(player_id)
    }

    pub fn insert_contract(&mut self, player_id: &str, contract: FarmingContract) {
        self.contracts.insert(player_id.to_string(), contract);
    }

    /// Restore a contract from database
    pub fn restore_contract(
        &mut self,
        player_id: &str,
        difficulty: &str,
        crop_id: &str,
        amount_required: i32,
        amount_harvested: i32,
        created_at: u64,
    ) {
        if let Some(diff) = ContractDifficulty::from_str(difficulty) {
            self.contracts.insert(
                player_id.to_string(),
                FarmingContract {
                    player_id: player_id.to_string(),
                    difficulty: diff,
                    crop_id: crop_id.to_string(),
                    amount_required,
                    amount_harvested,
                    created_at,
                },
            );
        }
    }

    /// Pick a random seed item appropriate for the player's level
    pub fn random_seed_for_level(&self, farming_level: i32) -> Option<String> {
        let eligible: Vec<&CropConfig> = self
            .crops
            .values()
            .filter(|c| c.level_required <= farming_level)
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        Some(eligible[rng.gen_range(0..eligible.len())].seed_item.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crop(seed_item: &str, level_required: i32) -> CropConfig {
        CropConfig {
            seed_item: seed_item.to_string(),
            produce_item: format!("{seed_item}_produce"),
            level_required,
            growth_time_minutes: 1.0,
            growth_stages: 4,
            harvest_amount_min: 1,
            harvest_amount_max: 1,
            xp_planting: 10,
            xp_per_harvest: 5,
            seed_return_chance: 0.0,
            category: "allotment".to_string(),
            lives_min: 1,
            lives_max: 1,
            disease_chance: 0.0,
        }
    }

    #[test]
    fn generate_contract_rejects_duplicate_active_contracts() {
        let mut system = FarmingSystem::new();
        system
            .crops
            .insert("carrot".to_string(), crop("carrot_seed", 1));

        let crop_id = {
            let contract = system
                .generate_contract("player", &ContractDifficulty::Easy, 1, 123)
                .unwrap();
            assert_eq!(contract.player_id, "player");
            assert_eq!(contract.difficulty, ContractDifficulty::Easy);
            assert!((3..=5).contains(&contract.amount_required));
            assert_eq!(contract.amount_harvested, 0);
            assert_eq!(contract.created_at, 123);
            contract.crop_id.clone()
        };

        assert_eq!(crop_id, "carrot");
        assert_eq!(
            system
                .generate_contract("player", &ContractDifficulty::Easy, 1, 124)
                .unwrap_err(),
            "You already have an active contract"
        );
    }

    #[test]
    fn record_contract_harvest_caps_progress_at_requirement() {
        let mut system = FarmingSystem::new();
        system.contracts.insert(
            "player".to_string(),
            FarmingContract {
                player_id: "player".to_string(),
                difficulty: ContractDifficulty::Easy,
                crop_id: "carrot".to_string(),
                amount_required: 5,
                amount_harvested: 3,
                created_at: 0,
            },
        );

        assert_eq!(
            system.record_contract_harvest("player", "carrot", 10),
            Some((5, 5, true))
        );
        assert_eq!(system.get_contract("player").unwrap().amount_harvested, 5);
        assert_eq!(system.record_contract_harvest("player", "carrot", 1), None);
    }

    #[test]
    fn restore_contract_ignores_unknown_difficulties() {
        let mut system = FarmingSystem::new();

        system.restore_contract("player", "legendary", "carrot", 3, 0, 0);

        assert!(system.get_contract("player").is_none());
    }

    #[test]
    fn random_seed_for_level_only_returns_eligible_seeds() {
        let mut system = FarmingSystem::new();
        system
            .crops
            .insert("carrot".to_string(), crop("carrot_seed", 1));
        system.crops.insert("yam".to_string(), crop("yam_seed", 30));

        assert_eq!(
            system.random_seed_for_level(1).as_deref(),
            Some("carrot_seed")
        );
        assert_eq!(system.random_seed_for_level(0), None);
    }
}
