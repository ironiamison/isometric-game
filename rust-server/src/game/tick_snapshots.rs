use super::{GameRoom, Player, PlayerStall, PlayerUpdate};
use crate::chunk::CHUNK_SIZE;
use std::collections::{HashMap, HashSet};

type PlayerPosition = (String, i32, i32, i32);
type ChunkPlayerEntry = (String, i32, i32);

pub(in crate::game) struct OverworldVisibilitySnapshot {
    pub player_positions: Vec<PlayerPosition>,
    pub players_by_chunk: HashMap<(i32, i32), Vec<ChunkPlayerEntry>>,
}

fn move_ack_seq(last_processed_move_seq: u32) -> Option<u32> {
    if last_processed_move_seq > 0 {
        Some(last_processed_move_seq)
    } else {
        None
    }
}

fn active_stall_name(stall: Option<&PlayerStall>) -> Option<String> {
    stall
        .filter(|stall| stall.active)
        .map(|stall| stall.name.clone())
}

fn player_update_from_player(
    player: &Player,
    is_gathering: bool,
    is_woodcutting: bool,
) -> PlayerUpdate {
    PlayerUpdate {
        id: player.id.clone(),
        name: player.name.clone(),
        x: player.x,
        y: player.y,
        z: player.z,
        direction: player.direction as u8,
        vel_x: player.move_dx,
        vel_y: player.move_dy,
        move_ack_seq: move_ack_seq(player.last_processed_move_seq),
        hp: player.hp,
        max_hp: player.max_hp(),
        combat_level: player.combat_level(),
        hitpoints_level: player.skills.hitpoints.level,
        attack_level: player.skills.attack.level,
        strength_level: player.skills.strength.level,
        defence_level: player.skills.defence.level,
        ranged_level: player.skills.ranged.level,
        gold: player.inventory.gold,
        gender: player.gender.clone(),
        skin: player.skin.clone(),
        hair_style: player.hair_style,
        hair_color: player.hair_color,
        equipped_head: player.equipped_head.clone(),
        equipped_body: player.equipped_body.clone(),
        equipped_weapon: player.equipped_weapon.clone(),
        equipped_back: player.equipped_back.clone(),
        equipped_feet: player.equipped_feet.clone(),
        equipped_ring: player.equipped_ring.clone(),
        equipped_gloves: player.equipped_gloves.clone(),
        equipped_necklace: player.equipped_necklace.clone(),
        equipped_belt: player.equipped_belt.clone(),
        is_admin: player.is_admin,
        sitting: player.sitting_at.is_some(),
        is_gathering,
        is_woodcutting,
        dashing: player.is_dashing,
        mp: player.mp,
        max_mp: player.max_mp(),
        has_stall: player.stall.as_ref().is_some_and(|stall| stall.active),
        stall_name: active_stall_name(player.stall.as_ref()),
        combat_style: player.combat_style.as_str().to_string(),
    }
}

fn chunk_key_for_position(x: i32, y: i32) -> (i32, i32) {
    (
        x.div_euclid(CHUNK_SIZE as i32),
        y.div_euclid(CHUNK_SIZE as i32),
    )
}

fn build_players_by_chunk(
    player_positions: &[(String, i32, i32, i32)],
) -> HashMap<(i32, i32), Vec<ChunkPlayerEntry>> {
    let mut players_by_chunk = HashMap::new();

    for (pid, px, py, _) in player_positions {
        players_by_chunk
            .entry(chunk_key_for_position(*px, *py))
            .or_insert_with(Vec::new)
            .push((pid.clone(), *px, *py));
    }

    players_by_chunk
}

impl GameRoom {
    pub(in crate::game) async fn collect_player_updates(
        &self,
        gathering_player_ids: &HashSet<String>,
        woodcutting_player_ids: &HashSet<String>,
    ) -> Vec<PlayerUpdate> {
        let players = self.players.read().await;

        players
            .values()
            .filter(|player| player.active)
            .map(|player| {
                player_update_from_player(
                    player,
                    gathering_player_ids.contains(&player.id),
                    woodcutting_player_ids.contains(&player.id),
                )
            })
            .collect()
    }

    pub(in crate::game) async fn collect_overworld_visibility_snapshot(
        &self,
    ) -> OverworldVisibilitySnapshot {
        let player_positions: Vec<PlayerPosition> = {
            let players = self.players.read().await;
            let gathering = self.gathering.read().await;
            let instances = self.player_instances.read().await;

            players
                .values()
                .filter(|player| player.active && player.is_alive())
                .filter(|player| !instances.contains_key(&player.id))
                .filter(|player| !gathering.is_gathering(&player.id))
                .map(|player| (player.id.clone(), player.x, player.y, player.hp))
                .collect()
        };

        let players_by_chunk = build_players_by_chunk(&player_positions);

        OverworldVisibilitySnapshot {
            player_positions,
            players_by_chunk,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_update_from_player_preserves_ack_and_active_stall() {
        let mut player = Player::new("char_1", "Alice", 15, 4, "female", "tan", Some(2), Some(1));
        player.active = true;
        player.move_dx = 1;
        player.move_dy = -1;
        player.last_processed_move_seq = 42;
        player.is_admin = true;
        player.is_dashing = true;
        player.stall = Some(PlayerStall {
            name: "Alice's Goods".to_string(),
            slots: Vec::new(),
            active: true,
        });

        let update = player_update_from_player(&player, true, false);

        assert_eq!(update.id, "char_1");
        assert_eq!(update.move_ack_seq, Some(42));
        assert!(update.is_gathering);
        assert!(!update.is_woodcutting);
        assert!(update.dashing);
        assert!(update.has_stall);
        assert_eq!(update.stall_name.as_deref(), Some("Alice's Goods"));
    }

    #[test]
    fn build_players_by_chunk_groups_players_by_chunk_coordinate() {
        let player_positions = vec![
            ("char_1".to_string(), 15, 4, 10),
            ("char_2".to_string(), 31, 31, 10),
            ("char_3".to_string(), 32, 0, 10),
        ];

        let players_by_chunk = build_players_by_chunk(&player_positions);

        assert_eq!(players_by_chunk.get(&(0, 0)).map(Vec::len), Some(2));
        assert_eq!(players_by_chunk.get(&(1, 0)).map(Vec::len), Some(1));
    }
}
