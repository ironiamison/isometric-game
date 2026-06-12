use super::*;
use crate::game::Player;
use crate::npc::{Npc, NpcState};

// ============================================================================
// Admin/Ops API view types (read-only snapshots of live game state)
// ============================================================================

#[derive(Serialize)]
pub(super) struct AdminRoomSummary {
    pub room_id: String,
    pub player_count: usize,
    pub npc_count: usize,
    pub overworld_players: usize,
    pub instance_players: usize,
}

#[derive(Serialize)]
pub(super) struct AdminPlayer {
    pub id: String,
    pub name: String,
    pub room_id: String,
    pub instance_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub combat_level: i32,
    pub active: bool,
    pub is_dead: bool,
    pub target_id: Option<String>,
    pub is_admin: bool,
    pub is_god_mode: bool,
    pub ip_address: Option<String>,
}

#[derive(Serialize)]
pub(super) struct AdminNpc {
    pub id: String,
    pub prototype_id: String,
    pub display_name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub level: i32,
    pub state: String,
    pub target_id: Option<String>,
    pub hidden: bool,
    pub invulnerable: bool,
}

#[derive(Serialize)]
pub(super) struct AdminRoomEntities {
    pub room_id: String,
    pub npcs: Vec<AdminNpc>,
    pub players: Vec<AdminPlayer>,
}

/// Stable string label for an NPC AI state (do not rely on Debug formatting,
/// which could change silently).
pub(super) fn npc_state_label(state: NpcState) -> &'static str {
    match state {
        NpcState::Idle => "Idle",
        NpcState::Chasing => "Chasing",
        NpcState::Attacking => "Attacking",
        NpcState::Returning => "Returning",
        NpcState::Dead => "Dead",
        NpcState::Wandering => "Wandering",
        NpcState::Submerging => "Submerging",
        NpcState::Emerging => "Emerging",
        NpcState::Burrowing => "Burrowing",
    }
}

/// Build an admin NPC view from a live NPC.
pub(super) fn admin_npc_from(npc: &Npc) -> AdminNpc {
    AdminNpc {
        id: npc.id.clone(),
        prototype_id: npc.prototype_id.clone(),
        display_name: npc.stats.display_name.clone(),
        x: npc.x,
        y: npc.y,
        z: npc.z,
        hp: npc.hp,
        max_hp: npc.max_hp,
        level: npc.level,
        state: npc_state_label(npc.state).to_string(),
        target_id: npc.target_id.clone(),
        hidden: npc.hidden,
        invulnerable: npc.invulnerable,
    }
}

/// Build an admin player view from a live player and the instance map snapshot.
pub(super) fn admin_player_from(
    player: &Player,
    room_id: &str,
    instance_id: Option<String>,
) -> AdminPlayer {
    AdminPlayer {
        id: player.id.clone(),
        name: player.name.clone(),
        room_id: room_id.to_string(),
        instance_id,
        x: player.x,
        y: player.y,
        z: player.z,
        hp: player.hp,
        max_hp: player.max_hp(),
        combat_level: player.skills.combat_level(),
        active: player.active,
        is_dead: player.is_dead,
        target_id: player.target_id.clone(),
        is_admin: player.is_admin,
        is_god_mode: player.is_god_mode,
        ip_address: player.ip_address.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_state_label_covers_all_variants() {
        assert_eq!(npc_state_label(NpcState::Idle), "Idle");
        assert_eq!(npc_state_label(NpcState::Chasing), "Chasing");
        assert_eq!(npc_state_label(NpcState::Attacking), "Attacking");
        assert_eq!(npc_state_label(NpcState::Returning), "Returning");
        assert_eq!(npc_state_label(NpcState::Dead), "Dead");
        assert_eq!(npc_state_label(NpcState::Wandering), "Wandering");
        assert_eq!(npc_state_label(NpcState::Submerging), "Submerging");
        assert_eq!(npc_state_label(NpcState::Emerging), "Emerging");
        assert_eq!(npc_state_label(NpcState::Burrowing), "Burrowing");
    }
}
