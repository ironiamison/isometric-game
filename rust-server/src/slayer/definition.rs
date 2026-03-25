use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SlayerTaskDef {
    pub monster_id: String,
    pub display_name: String,
    pub count_min: i32,
    pub count_max: i32,
    #[serde(default = "default_weight")]
    pub weight: i32,
    #[serde(default)]
    pub slayer_level_required: i32,
    #[serde(default)]
    pub requires_unlock: bool,
    #[serde(default = "default_xp_per_kill")]
    pub xp_per_kill: i64,
    /// Additional monster IDs that count for this task (e.g. piglet counts for pig)
    #[serde(default)]
    pub aliases: Vec<String>,
}

fn default_weight() -> i32 {
    10
}
fn default_xp_per_kill() -> i64 {
    15
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlayerMasterDef {
    pub id: String,
    pub display_name: String,
    pub entity_prototype: String,
    #[serde(default)]
    pub combat_level_required: i32,
    #[serde(default)]
    pub slayer_level_required: i32,
    #[serde(default = "default_points_per_task")]
    pub points_per_task: i32,
    pub tasks: Vec<SlayerTaskDef>,
}

fn default_points_per_task() -> i32 {
    2
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlayerRewardDef {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub cost: i32,
    pub category: String,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default = "default_quantity")]
    pub quantity: i32,
}

fn default_quantity() -> i32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlayerMastersFile {
    pub masters: Vec<SlayerMasterDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlayerRewardsFile {
    pub rewards: Vec<SlayerRewardDef>,
}
