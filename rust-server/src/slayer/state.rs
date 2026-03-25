use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlayerTask {
    pub monster_id: String,
    pub display_name: String,
    pub kills_required: i32,
    pub kills_current: i32,
    pub xp_per_kill: i64,
    pub master_id: String,
    pub points_on_complete: i32,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerSlayerState {
    pub current_task: Option<SlayerTask>,
    pub points: i32,
    pub tasks_completed: i32,
    pub blocked_monsters: Vec<String>,
    pub unlocked_monsters: Vec<String>,
}
