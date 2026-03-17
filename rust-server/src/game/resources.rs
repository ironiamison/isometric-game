use super::{Direction, GameRoom};
use crate::protocol::ServerMessage;
use std::collections::HashMap;

fn direction_step(direction: Direction) -> (i32, i32) {
    match direction {
        Direction::Down => (0, 1),
        Direction::Up => (0, -1),
        Direction::Left => (-1, 0),
        Direction::Right => (1, 0),
        Direction::DownLeft => (-1, 1),
        Direction::DownRight => (1, 1),
        Direction::UpLeft => (-1, -1),
        Direction::UpRight => (1, -1),
    }
}

fn is_marker_in_front(
    player_x: i32,
    player_y: i32,
    player_dir: Direction,
    marker_x: i32,
    marker_y: i32,
) -> bool {
    let (dx, dy) = direction_step(player_dir);
    player_x + dx == marker_x && player_y + dy == marker_y
}

fn is_adjacent_resource(player_x: i32, player_y: i32, target_x: i32, target_y: i32) -> bool {
    let dx = (player_x - target_x).abs();
    let dy = (player_y - target_y).abs();
    dx <= 1 && dy <= 1 && (dx != 0 || dy != 0)
}

fn has_fishing_rod(equipped_weapon: Option<&str>) -> bool {
    matches!(equipped_weapon, Some("fishing_rod" | "maple_rod"))
}

impl GameRoom {
    /// Register gathering markers for an instance (called when instance is created)
    pub async fn register_instance_gathering_markers(
        &self,
        instance_id: &str,
        markers: Vec<crate::gathering::GatheringMarker>,
    ) {
        let mut gathering = self.gathering.write().await;
        gathering.register_instance_markers(instance_id, markers);
    }

    pub async fn get_gathering_markers_message(&self, instance_id: Option<&str>) -> ServerMessage {
        let gathering = self.gathering.read().await;
        let source_markers: &[crate::gathering::GatheringMarker] = if let Some(inst_id) = instance_id {
            gathering.get_instance_markers(inst_id)
        } else {
            &gathering.markers
        };
        let markers = source_markers
            .iter()
            .map(|marker| {
                let skill = gathering
                    .zones
                    .get(&marker.zone_id)
                    .map(|zone| zone.skill.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                crate::protocol::GatheringMarkerData {
                    x: marker.x,
                    y: marker.y,
                    zone_id: marker.zone_id.clone(),
                    skill,
                }
            })
            .collect();
        ServerMessage::GatheringMarkers { markers }
    }

    pub async fn handle_start_gathering(&self, player_id: &str, marker_x: i32, marker_y: i32) {
        let instance_id = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (fishing_level, player_x, player_y, player_dir, equipped_weapon) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (
                    player.skills.fishing.level,
                    player.x,
                    player.y,
                    player.direction,
                    player.equipped_weapon.clone(),
                ),
                None => return,
            }
        };

        if !is_marker_in_front(player_x, player_y, player_dir, marker_x, marker_y) {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You must face the gathering spot".to_string(),
                },
            )
            .await;
            return;
        }

        {
            let gathering = self.gathering.read().await;
            if let Some(zone) = gathering.get_zone_for_marker(instance_id.as_deref(), marker_x, marker_y) {
                if zone.skill.as_str() == "fishing" && !has_fishing_rod(equipped_weapon.as_deref())
                {
                    drop(gathering);
                    self.send_to_player(
                        player_id,
                        ServerMessage::Error {
                            code: 400,
                            message: "You need a fishing rod to do that".to_string(),
                        },
                    )
                    .await;
                    return;
                }
            }
        }

        let mut gathering = self.gathering.write().await;
        match gathering.start_gathering(player_id, instance_id.as_deref(), marker_x, marker_y, fishing_level, current_time)
        {
            Ok(zone_id) => {
                self.broadcast(ServerMessage::GatheringStarted {
                    player_id: player_id.to_string(),
                    marker_x,
                    marker_y,
                    zone_id,
                })
                .await;
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_stop_gathering(&self, player_id: &str) {
        let mut gathering = self.gathering.write().await;
        if gathering.stop_gathering(player_id).is_some() {
            self.broadcast(ServerMessage::GatheringStopped {
                player_id: player_id.to_string(),
                reason: "cancelled".to_string(),
            })
            .await;
        }
    }

    /// Handle a single chop attempt on a tree.
    pub async fn handle_chop_tree(&self, player_id: &str, tree_x: i32, tree_y: i32, tree_gid: u32) {
        let instance_id = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (woodcutting_level, player_x, player_y, equipped_weapon) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (
                    player.skills.woodcutting.level,
                    player.x,
                    player.y,
                    player.equipped_weapon.clone(),
                ),
                None => return,
            }
        };

        if !is_adjacent_resource(player_x, player_y, tree_x, tree_y) {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need to be next to the tree".to_string(),
                },
            )
            .await;
            return;
        }

        let axe_success_bonus = if let Some(ref weapon_id) = equipped_weapon {
            if let Some(item_def) = self.item_registry.get(weapon_id) {
                if let Some(ref equip) = item_def.equipment {
                    if equip.chop_speed_multiplier > 0.0 {
                        if equip.woodcutting_level_required > woodcutting_level {
                            self.send_to_player(
                                player_id,
                                ServerMessage::Error {
                                    code: 400,
                                    message: format!(
                                        "You need Woodcutting level {} to use this axe",
                                        equip.woodcutting_level_required
                                    ),
                                },
                            )
                            .await;
                            return;
                        }
                        Some(equip.chop_success_bonus)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if axe_success_bonus.is_none() {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need an axe to chop trees".to_string(),
                },
            )
            .await;
            return;
        }

        {
            let woodcutting = self.woodcutting.read().await;
            if let Some(tree_config) = woodcutting.get_tree_type(tree_gid) {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    if !player.inventory.has_space_for(
                        &tree_config.log_item_id,
                        1,
                        &self.item_registry,
                    ) {
                        drop(players);
                        drop(woodcutting);
                        self.send_to_player(
                            player_id,
                            ServerMessage::Error {
                                code: 400,
                                message: "Your inventory is full!".to_string(),
                            },
                        )
                        .await;
                        return;
                    }
                }
            }
        }

        let mut woodcutting = self.woodcutting.write().await;
        let chop_result = woodcutting.chop_once(
            instance_id.as_deref(),
            tree_x,
            tree_y,
            tree_gid,
            woodcutting_level,
            axe_success_bonus.unwrap_or(0.0),
            current_time,
        );
        drop(woodcutting);

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.last_attack_time = current_time;
            }
        }

        match chop_result {
            Ok(result) => {
                self.broadcast(ServerMessage::WoodcuttingSwing {
                    player_id: player_id.to_string(),
                    tree_x,
                    tree_y,
                })
                .await;

                if result.success {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        let leftover =
                            player
                                .inventory
                                .add_item(&result.log_item_id, 1, &self.item_registry);
                        if leftover > 0 {
                            drop(players);
                            self.send_to_player(
                                player_id,
                                ServerMessage::Error {
                                    code: 400,
                                    message: "Your inventory is full!".to_string(),
                                },
                            )
                            .await;
                            return;
                        }

                        let leveled_up = player.skills.woodcutting.add_xp(result.xp_gained);
                        let new_xp = player.skills.woodcutting.xp;
                        let new_level = player.skills.woodcutting.level;
                        let inv_update = player.inventory.to_update();
                        let gold = player.inventory.gold;
                        drop(players);

                        self.send_to_player(
                            player_id,
                            ServerMessage::InventoryUpdate {
                                player_id: player_id.to_string(),
                                slots: inv_update,
                                gold,
                            },
                        )
                        .await;

                        self.send_to_player(
                            player_id,
                            ServerMessage::SkillXp {
                                player_id: player_id.to_string(),
                                skill: "woodcutting".to_string(),
                                xp_gained: result.xp_gained,
                                total_xp: new_xp,
                                level: new_level,
                            },
                        )
                        .await;

                        self.send_to_player(
                            player_id,
                            ServerMessage::WoodcuttingResult {
                                player_id: player_id.to_string(),
                                item_id: result.log_item_id.clone(),
                                xp_gained: result.xp_gained,
                            },
                        )
                        .await;

                        self.process_quest_item_collect(player_id, &result.log_item_id, 1)
                            .await;

                        if leveled_up {
                            self.broadcast_skill_level_up(player_id, "woodcutting", new_level).await;
                            self.process_quest_progression_snapshot(player_id).await;
                        }
                    }
                }

                if result.tree_depleted {
                    let respawn_delay = result.respawn_delay_ms.unwrap_or(7500);
                    self.broadcast(ServerMessage::TreeDepleted {
                        x: tree_x,
                        y: tree_y,
                        gid: tree_gid,
                        respawn_delay_ms: respawn_delay,
                    })
                    .await;

                    self.process_quest_tree_deplete(
                        player_id,
                        &result.tree_type_id,
                        tree_x,
                        tree_y,
                    )
                    .await;
                }
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub async fn handle_mine_rock(&self, player_id: &str, rock_x: i32, rock_y: i32, rock_gid: u32) {
        let instance_id = self.player_instances.read().await.get(player_id).cloned();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let (mining_level, player_x, player_y, equipped_weapon) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (
                    player.skills.mining.level,
                    player.x,
                    player.y,
                    player.equipped_weapon.clone(),
                ),
                None => return,
            }
        };

        if !is_adjacent_resource(player_x, player_y, rock_x, rock_y) {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need to be next to the rock".to_string(),
                },
            )
            .await;
            return;
        }

        let pickaxe_success_bonus = if let Some(ref weapon_id) = equipped_weapon {
            if let Some(item_def) = self.item_registry.get(weapon_id) {
                if let Some(ref equip) = item_def.equipment {
                    if equip.mine_speed_multiplier > 0.0 {
                        if equip.mining_level_required > mining_level {
                            self.send_to_player(
                                player_id,
                                ServerMessage::Error {
                                    code: 400,
                                    message: format!(
                                        "You need Mining level {} to use this pickaxe",
                                        equip.mining_level_required
                                    ),
                                },
                            )
                            .await;
                            return;
                        }
                        Some(equip.mine_success_bonus)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if pickaxe_success_bonus.is_none() {
            self.send_to_player(
                player_id,
                ServerMessage::Error {
                    code: 400,
                    message: "You need a pickaxe to mine rocks".to_string(),
                },
            )
            .await;
            return;
        }

        {
            let mining = self.mining.read().await;
            if let Some(ore_config) = mining.get_ore_type(rock_gid) {
                let players = self.players.read().await;
                if let Some(player) = players.get(player_id) {
                    if !player.inventory.has_space_for(
                        &ore_config.ore_item_id,
                        1,
                        &self.item_registry,
                    ) {
                        drop(players);
                        drop(mining);
                        self.send_to_player(
                            player_id,
                            ServerMessage::Error {
                                code: 400,
                                message: "Your inventory is full!".to_string(),
                            },
                        )
                        .await;
                        return;
                    }
                }
            }
        }

        let mut mining = self.mining.write().await;
        let mine_result = mining.mine_once(
            instance_id.as_deref(),
            rock_x,
            rock_y,
            rock_gid,
            mining_level,
            pickaxe_success_bonus.unwrap_or(0.0),
            current_time,
        );
        drop(mining);

        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.last_attack_time = current_time;
            }
        }

        match mine_result {
            Ok(result) => {
                self.broadcast(ServerMessage::MiningSwing {
                    player_id: player_id.to_string(),
                    rock_x,
                    rock_y,
                })
                .await;

                if result.success {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        let leftover =
                            player
                                .inventory
                                .add_item(&result.ore_item_id, 1, &self.item_registry);
                        if leftover > 0 {
                            drop(players);
                            self.send_to_player(
                                player_id,
                                ServerMessage::Error {
                                    code: 400,
                                    message: "Your inventory is full!".to_string(),
                                },
                            )
                            .await;
                            return;
                        }

                        let gem_dropped_on_ground = if let Some(ref gem_id) = result.gem_drop {
                            player.inventory.add_item(gem_id, 1, &self.item_registry) > 0
                        } else {
                            false
                        };

                        let leveled_up = player.skills.mining.add_xp(result.xp_gained);
                        let new_xp = player.skills.mining.xp;
                        let new_level = player.skills.mining.level;
                        let inv_update = player.inventory.to_update();
                        let gold = player.inventory.gold;
                        let player_x = player.x;
                        let player_y = player.y;
                        drop(players);

                        self.send_to_player(
                            player_id,
                            ServerMessage::InventoryUpdate {
                                player_id: player_id.to_string(),
                                slots: inv_update,
                                gold,
                            },
                        )
                        .await;

                        self.send_to_player(
                            player_id,
                            ServerMessage::SkillXp {
                                player_id: player_id.to_string(),
                                skill: "mining".to_string(),
                                xp_gained: result.xp_gained,
                                total_xp: new_xp,
                                level: new_level,
                            },
                        )
                        .await;

                        self.send_to_player(
                            player_id,
                            ServerMessage::MiningResult {
                                player_id: player_id.to_string(),
                                item_id: result.ore_item_id.clone(),
                                xp_gained: result.xp_gained,
                            },
                        )
                        .await;

                        if let Some(ref gem_id) = result.gem_drop {
                            let gem_name = self
                                .item_registry
                                .get(gem_id)
                                .map(|item| item.display_name.clone())
                                .unwrap_or_else(|| gem_id.clone());

                            if gem_dropped_on_ground {
                                let ground_item = crate::item::GroundItem::new(
                                    &uuid::Uuid::new_v4().to_string(),
                                    gem_id,
                                    player_x as f32,
                                    player_y as f32,
                                    1,
                                    Some(player_id.to_string()),
                                    current_time,
                                );

                                self.broadcast_to_zone(
                                    player_id,
                                    ServerMessage::ItemDropped {
                                        id: ground_item.id.clone(),
                                        item_id: gem_id.clone(),
                                        x: player_x as f32,
                                        y: player_y as f32,
                                        quantity: 1,
                                    },
                                )
                                .await;

                                {
                                    let mut items = self.ground_items.write().await;
                                    items.insert(ground_item.id.clone(), ground_item);
                                }

                                self.send_system_message(
                                    player_id,
                                    &format!(
                                        "You found a {}! Your inventory is full, so it dropped on the ground.",
                                        gem_name,
                                    ),
                                )
                                .await;
                            } else {
                                self.send_system_message(
                                    player_id,
                                    &format!("You found a {} while mining!", gem_name),
                                )
                                .await;
                            }
                        }

                        self.process_quest_item_collect(player_id, &result.ore_item_id, 1)
                            .await;

                        if leveled_up {
                            self.broadcast_skill_level_up(player_id, "mining", new_level).await;
                            self.process_quest_progression_snapshot(player_id).await;
                        }
                    }
                }

                if result.rock_depleted {
                    let respawn_delay = result.respawn_delay_ms.unwrap_or(7500);
                    self.broadcast(ServerMessage::RockDepleted {
                        x: rock_x,
                        y: rock_y,
                        gid: rock_gid,
                        respawn_delay_ms: respawn_delay,
                    })
                    .await;

                    self.process_quest_rock_deplete(player_id, &result.ore_type_id, rock_x, rock_y)
                        .await;
                }
            }
            Err(message) => {
                self.send_to_player(player_id, ServerMessage::Error { code: 400, message })
                    .await;
            }
        }
    }

    pub(in crate::game) async fn process_resource_ticks(&self, current_time: u64) {
        struct GatherTick {
            pid: String,
            item_id: String,
            xp_gained: i64,
            total_xp: i64,
            level: i32,
            leveled: bool,
            inv_update: Vec<crate::item::InventorySlotUpdate>,
            gold: i32,
        }

        struct GatherResult {
            pid: String,
            item_id: String,
            xp_gained: i64,
        }

        let gatherer_stats: HashMap<String, (i32, f32, f32)> = {
            let players = self.players.read().await;
            let gathering = self.gathering.read().await;
            gathering
                .player_states
                .keys()
                .filter_map(|player_id| {
                    players.get(player_id).map(|player| {
                        let active_ids: Vec<String> =
                            player.active_prayers.iter().cloned().collect();
                        let effects = self.prayer_registry.calculate_effects(&active_ids);
                        let rod_speed = player
                            .equipped_weapon
                            .as_ref()
                            .and_then(|weapon| self.item_registry.get(weapon))
                            .and_then(|definition| definition.equipment.as_ref())
                            .map(|equipment| equipment.fishing_speed_multiplier)
                            .unwrap_or(1.0)
                            .max(1.0);

                        (
                            player_id.clone(),
                            (
                                player.skills.fishing.level,
                                effects.gather_speed_multiplier(),
                                rod_speed,
                            ),
                        )
                    })
                })
                .collect()
        };

        let (gather_results, bonus_events) = {
            let mut gathering = self.gathering.write().await;
            let mut results: Vec<GatherResult> = Vec::new();
            let gatherer_ids: Vec<String> = gathering.player_states.keys().cloned().collect();
            for player_id in &gatherer_ids {
                let (fishing_level, prayer_speed, rod_speed) = gatherer_stats
                    .get(player_id)
                    .copied()
                    .unwrap_or((1, 1.0, 1.0));
                if let Some(result) = gathering.tick_gathering(
                    player_id,
                    fishing_level,
                    current_time,
                    prayer_speed,
                    rod_speed,
                ) {
                    results.push(GatherResult {
                        pid: player_id.clone(),
                        item_id: result.item_id,
                        xp_gained: result.xp_gained,
                    });
                }
            }
            let bonus_events = gathering.tick_bonus_tiles(current_time);
            (results, bonus_events)
        };

        let mut inventory_full_players: Vec<String> = Vec::new();
        let gather_ticks = {
            let mut players = self.players.write().await;
            let mut ticks: Vec<GatherTick> = Vec::new();
            for result in &gather_results {
                if let Some(player) = players.get_mut(&result.pid) {
                    let leftover =
                        player
                            .inventory
                            .add_item(&result.item_id, 1, &self.item_registry);
                    if leftover > 0 {
                        inventory_full_players.push(result.pid.clone());
                        continue;
                    }

                    let leveled = player.skills.fishing.add_xp(result.xp_gained);
                    ticks.push(GatherTick {
                        pid: result.pid.clone(),
                        item_id: result.item_id.clone(),
                        xp_gained: result.xp_gained,
                        total_xp: player.skills.fishing.xp,
                        level: player.skills.fishing.level,
                        leveled,
                        inv_update: player.inventory.to_update(),
                        gold: player.inventory.gold,
                    });
                }
            }
            ticks
        };

        if !inventory_full_players.is_empty() {
            let mut gathering = self.gathering.write().await;
            for player_id in &inventory_full_players {
                gathering.stop_gathering(player_id);
            }
        }

        for tick in gather_ticks {
            self.process_quest_item_collect(&tick.pid, &tick.item_id, 1)
                .await;

            self.send_to_player(
                &tick.pid,
                ServerMessage::GatheringResult {
                    player_id: tick.pid.clone(),
                    item_id: tick.item_id,
                    xp_gained: tick.xp_gained,
                },
            )
            .await;
            self.send_to_player(
                &tick.pid,
                ServerMessage::InventoryUpdate {
                    player_id: tick.pid.clone(),
                    slots: tick.inv_update,
                    gold: tick.gold,
                },
            )
            .await;
            self.send_to_player(
                &tick.pid,
                ServerMessage::SkillXp {
                    player_id: tick.pid.clone(),
                    skill: "fishing".to_string(),
                    xp_gained: tick.xp_gained,
                    total_xp: tick.total_xp,
                    level: tick.level,
                },
            )
            .await;
            if tick.leveled {
                self.broadcast_skill_level_up(&tick.pid, "fishing", tick.level).await;
                self.process_quest_progression_snapshot(&tick.pid).await;
            }
        }

        for player_id in inventory_full_players {
            self.broadcast(ServerMessage::GatheringStopped {
                player_id,
                reason: "inventory_full".to_string(),
            })
            .await;
        }

        for event in bonus_events {
            match event {
                crate::gathering::BonusTileEvent::Spawned { x, y, zone_id } => {
                    self.send_to_overworld_players(
                        ServerMessage::BonusTileSpawned {
                            x,
                            y,
                            zone_id,
                            telegraph_duration: 5000,
                        },
                        None,
                    )
                    .await;
                }
                crate::gathering::BonusTileEvent::Expired { x, y } => {
                    self.send_to_overworld_players(ServerMessage::BonusTileExpired { x, y }, None)
                        .await;
                }
            }
        }

        let tree_respawn_events = {
            let mut woodcutting = self.woodcutting.write().await;
            woodcutting.tick_respawns(current_time)
        };
        for event in tree_respawn_events {
            self.broadcast(ServerMessage::TreeRespawned {
                x: event.x,
                y: event.y,
                gid: event.gid,
            })
            .await;
        }

        let rock_respawn_events = {
            let mut mining = self.mining.write().await;
            mining.tick_respawns(current_time)
        };
        for event in rock_respawn_events {
            self.broadcast(ServerMessage::RockRespawned {
                x: event.x,
                y: event.y,
                gid: event.gid,
            })
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_in_front_matches_cardinal_and_diagonal_steps() {
        assert!(is_marker_in_front(10, 10, Direction::Down, 10, 11));
        assert!(is_marker_in_front(10, 10, Direction::Left, 9, 10));
        assert!(is_marker_in_front(10, 10, Direction::UpRight, 11, 9));
        assert!(!is_marker_in_front(10, 10, Direction::Up, 10, 11));
    }

    #[test]
    fn adjacent_resource_requires_neighbor_tile() {
        assert!(is_adjacent_resource(10, 10, 9, 9));
        assert!(is_adjacent_resource(10, 10, 10, 11));
        assert!(!is_adjacent_resource(10, 10, 10, 10));
        assert!(!is_adjacent_resource(10, 10, 12, 10));
    }

    #[test]
    fn fishing_rod_check_only_accepts_supported_rods() {
        assert!(has_fishing_rod(Some("fishing_rod")));
        assert!(has_fishing_rod(Some("maple_rod")));
        assert!(!has_fishing_rod(Some("bronze_sword")));
        assert!(!has_fishing_rod(None));
    }
}
