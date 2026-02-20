use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlayerTaskClientData {
    pub monster_id: String,
    pub display_name: String,
    pub kills_current: i32,
    pub kills_required: i32,
    pub xp_per_kill: i64,
    pub master_id: String,
    pub points_on_complete: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlayerRewardClientData {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub cost: i32,
    pub category: String,
    pub target_id: Option<String>,
    pub quantity: i32,
}
