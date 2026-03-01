use super::*;

const ALTAR_INTERACTION_DISTANCE: f32 = 2.5;

#[derive(Debug, PartialEq, Eq)]
enum PrayerToggleAction {
    Activated { replaced: Option<String> },
    Deactivated,
}

fn altar_dialogue_text(current_points: i32, max_points: i32) -> String {
    if current_points < max_points {
        format!(
            "You stand before the sacred altar.\n\nPrayer Points: {}/{}\n\nWould you like to pray and restore your prayer points?",
            current_points, max_points
        )
    } else {
        format!(
            "You stand before the sacred altar.\n\nPrayer Points: {}/{}\n\nYour prayer is already full. You may offer bones here for enhanced experience.",
            current_points, max_points
        )
    }
}

fn altar_xp_for_item(item_id: &str, base_prayer_xp: i32) -> i32 {
    match item_id {
        "regular_bones" => 12,
        "big_bones" => 37,
        "dragon_bones" => 180,
        _ => (base_prayer_xp as f32 * 2.5) as i32,
    }
}

fn prayer_state_update(player: &Player) -> ServerMessage {
    ServerMessage::PrayerStateUpdate {
        points: player.prayer_points,
        max_points: player.max_prayer_points(),
        active_prayers: player.active_prayers.iter().cloned().collect(),
    }
}

fn inventory_update_message(player_id: &str, player: &Player) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        player_id: player_id.to_string(),
        slots: player.inventory.to_update(),
        gold: player.inventory.gold,
    }
}

fn prayer_xp_message(player_id: &str, xp_gained: i64, total_xp: i64, level: i32) -> ServerMessage {
    ServerMessage::SkillXp {
        player_id: player_id.to_string(),
        skill: "prayer".to_string(),
        xp_gained,
        total_xp,
        level,
    }
}

fn apply_prayer_toggle(
    active_prayers: &mut HashSet<String>,
    prayer_id: &str,
    prayer_category: crate::prayer::PrayerCategory,
    prayer_registry: &PrayerRegistry,
) -> PrayerToggleAction {
    if active_prayers.contains(prayer_id) {
        active_prayers.remove(prayer_id);
        return PrayerToggleAction::Deactivated;
    }

    let replaced = active_prayers.iter().find_map(|active_id| {
        prayer_registry
            .get(active_id)
            .filter(|prayer| prayer.category == prayer_category)
            .map(|_| active_id.clone())
    });

    if let Some(conflict_id) = &replaced {
        active_prayers.remove(conflict_id);
    }

    active_prayers.insert(prayer_id.to_string());
    PrayerToggleAction::Activated { replaced }
}

impl GameRoom {
    pub(super) async fn show_altar_dialogue(
        &self,
        player_id: &str,
        npc_id: &str,
        entity_type: &str,
    ) {
        let (current_points, max_points) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) => (player.prayer_points, player.max_prayer_points()),
                None => return,
            }
        };

        let altar_name = self
            .entity_registry
            .get(entity_type)
            .map(|p| p.display_name.clone())
            .unwrap_or_else(|| "Altar".to_string());

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("altar:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: altar_name,
                text: altar_dialogue_text(current_points, max_points),
                choices: vec![
                    crate::protocol::DialogueChoice {
                        id: "pray".to_string(),
                        text: "Pray".to_string(),
                    },
                    crate::protocol::DialogueChoice {
                        id: "close".to_string(),
                        text: "Close".to_string(),
                    },
                ],
            },
        )
        .await;
    }

    pub(super) async fn handle_altar_dialogue_choice(
        &self,
        player_id: &str,
        altar_id: &str,
        choice_id: &str,
    ) {
        self.send_to_player(player_id, ServerMessage::DialogueClosed)
            .await;
        if choice_id == "pray" {
            self.handle_pray_at_altar(player_id, altar_id).await;
        }
    }

    pub async fn get_player_prayer_state(&self, player_id: &str) -> Option<ServerMessage> {
        let players = self.players.read().await;
        players.get(player_id).map(prayer_state_update)
    }

    async fn send_prayer_level_up_updates(&self, player_id: &str, new_level: i32) {
        tracing::info!("Player {} leveled up Prayer to {}", player_id, new_level);
        self.broadcast_skill_level_up(player_id, "prayer", new_level).await;

        if let Some(state) = self.get_player_prayer_state(player_id).await {
            self.send_to_player(player_id, state).await;
        }

        self.process_quest_progression_snapshot(player_id).await;
    }

    async fn validate_altar_access(&self, player_id: &str, altar_id: &str) -> bool {
        let player_pos = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => (player.x, player.y),
                _ => return false,
            }
        };

        let is_in_instance = {
            let instances = self.player_instances.read().await;
            instances.contains_key(player_id)
        };

        let altar_info = if is_in_instance {
            if let Some(instance) = self.instance_manager.find_player_instance(player_id).await {
                let npcs = instance.npcs.read().await;
                npcs.get(altar_id).map(|npc| {
                    let dx = (npc.x - player_pos.0) as f32;
                    let dy = (npc.y - player_pos.1) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    (npc.prototype_id.clone(), distance)
                })
            } else {
                None
            }
        } else {
            let npcs = self.npcs.read().await;
            npcs.get(altar_id).map(|npc| {
                let dx = (npc.x - player_pos.0) as f32;
                let dy = (npc.y - player_pos.1) as f32;
                let distance = (dx * dx + dy * dy).sqrt();
                (npc.prototype_id.clone(), distance)
            })
        };

        let (entity_type, distance) = match altar_info {
            Some(info) => info,
            None => {
                self.send_system_message(player_id, "Altar not found.")
                    .await;
                return false;
            }
        };

        if distance > ALTAR_INTERACTION_DISTANCE {
            self.send_system_message(player_id, "You need to be closer to the altar.")
                .await;
            return false;
        }

        let is_altar = self
            .entity_registry
            .get(&entity_type)
            .map(|proto| proto.behaviors.altar)
            .unwrap_or(false);
        if !is_altar {
            self.send_system_message(player_id, "That's not an altar.")
                .await;
            return false;
        }

        true
    }

    pub async fn handle_toggle_prayer(&self, player_id: &str, prayer_id: &str) {
        let prayer = match self.prayer_registry.get(prayer_id) {
            Some(prayer) => prayer.clone(),
            None => {
                tracing::warn!(
                    "Player {} tried to toggle unknown prayer: {}",
                    player_id,
                    prayer_id
                );
                return;
            }
        };

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(player) => player,
            None => return,
        };
        if player.is_dead {
            return;
        }

        if !player.active_prayers.contains(prayer_id) {
            if player.skills.prayer.level < prayer.level_req {
                drop(players);
                self.send_system_message(
                    player_id,
                    &format!(
                        "You need Prayer level {} to use {}",
                        prayer.level_req, prayer.name
                    ),
                )
                .await;
                return;
            }

            if player.prayer_points <= 0 {
                drop(players);
                self.send_system_message(player_id, "You have no prayer points remaining")
                    .await;
                return;
            }
        }

        let action = apply_prayer_toggle(
            &mut player.active_prayers,
            prayer_id,
            prayer.category,
            &self.prayer_registry,
        );

        match &action {
            PrayerToggleAction::Deactivated => {
                tracing::debug!("Player {} deactivated prayer: {}", player_id, prayer_id);
            }
            PrayerToggleAction::Activated { replaced } => {
                if let Some(conflict_id) = replaced {
                    tracing::debug!(
                        "Player {} deactivated conflicting prayer {} to activate {}",
                        player_id,
                        conflict_id,
                        prayer_id
                    );
                }
                tracing::debug!("Player {} activated prayer: {}", player_id, prayer_id);
            }
        }

        let state = prayer_state_update(player);
        drop(players);
        self.send_to_player(player_id, state).await;
    }

    pub async fn handle_bury_bones(&self, player_id: &str, slot: usize) {
        tracing::debug!("Player {} burying bones from slot: {}", player_id, slot);

        let (item_name, prayer_xp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) => player,
                None => return,
            };

            let slot_item = match player.inventory.slots.get(slot).and_then(|s| s.as_ref()) {
                Some(item) => item,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "There's nothing in that slot.")
                        .await;
                    return;
                }
            };

            let item_def = match self.item_registry.get(&slot_item.item_id) {
                Some(def) => def,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "Unknown item.").await;
                    return;
                }
            };

            if !item_def.is_bones() {
                drop(players);
                self.send_system_message(player_id, "You can only bury bones.")
                    .await;
                return;
            }

            (item_def.display_name.clone(), item_def.prayer_xp)
        };

        let (inv_msg, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            if let Some(ref mut slot_item) = player.inventory.slots[slot] {
                slot_item.quantity -= 1;
                if slot_item.quantity <= 0 {
                    player.inventory.slots[slot] = None;
                }
            }

            let leveled = player.skills.prayer.add_xp(prayer_xp as i64);
            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            (
                inventory_update_message(player_id, player),
                (
                    prayer_xp as i64,
                    player.skills.prayer.xp,
                    player.skills.prayer.level,
                    leveled,
                ),
            )
        };

        self.send_system_message(
            player_id,
            &format!("You bury the {}.", item_name.to_lowercase()),
        )
        .await;
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            prayer_xp_message(player_id, xp_result.0, xp_result.1, xp_result.2),
        )
        .await;

        if xp_result.3 {
            self.send_prayer_level_up_updates(player_id, xp_result.2)
                .await;
        }
    }

    pub async fn handle_pray_at_altar(&self, player_id: &str, altar_id: &str) {
        tracing::debug!("Player {} praying at altar: {}", player_id, altar_id);

        if !self.validate_altar_access(player_id, altar_id).await {
            return;
        }

        let (restored, state) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            let old_points = player.prayer_points;
            let max_points = player.max_prayer_points();
            if player.prayer_points < max_points {
                player.prayer_points = max_points;
            }

            (
                player.prayer_points - old_points,
                prayer_state_update(player),
            )
        };

        if restored > 0 {
            self.send_system_message(
                player_id,
                &format!(
                    "You pray at the altar. Your prayer points have been restored. (+{} points)",
                    restored
                ),
            )
            .await;
        } else {
            self.send_system_message(
                player_id,
                "You pray at the altar. Your prayer is already full.",
            )
            .await;
        }

        self.send_to_player(player_id, state).await;
    }

    pub async fn handle_offer_bones(&self, player_id: &str, slot: usize, altar_id: &str) {
        tracing::debug!(
            "Player {} offering bones at altar {} from slot {}",
            player_id,
            altar_id,
            slot
        );

        if !self.validate_altar_access(player_id, altar_id).await {
            return;
        }

        let (item_id, base_prayer_xp) = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) => player,
                None => return,
            };

            let slot_item = match player.inventory.slots.get(slot).and_then(|s| s.as_ref()) {
                Some(item) => item,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "There's nothing in that slot.")
                        .await;
                    return;
                }
            };

            let item_def = match self.item_registry.get(&slot_item.item_id) {
                Some(def) => def,
                None => {
                    drop(players);
                    self.send_system_message(player_id, "Unknown item.").await;
                    return;
                }
            };

            if !item_def.is_bones() {
                drop(players);
                self.send_system_message(player_id, "You can only offer bones at the altar.")
                    .await;
                return;
            }

            (slot_item.item_id.clone(), item_def.prayer_xp)
        };

        let altar_xp = altar_xp_for_item(&item_id, base_prayer_xp);

        let (inv_msg, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            if let Some(ref mut slot_item) = player.inventory.slots[slot] {
                slot_item.quantity -= 1;
                if slot_item.quantity <= 0 {
                    player.inventory.slots[slot] = None;
                }
            }

            let leveled = player.skills.prayer.add_xp(altar_xp as i64);
            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            (
                inventory_update_message(player_id, player),
                (
                    altar_xp as i64,
                    player.skills.prayer.xp,
                    player.skills.prayer.level,
                    leveled,
                ),
            )
        };

        self.send_system_message(
            player_id,
            &format!(
                "The gods are pleased with your offering. (+{} Prayer XP)",
                altar_xp
            ),
        )
        .await;
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            prayer_xp_message(player_id, xp_result.0, xp_result.1, xp_result.2),
        )
        .await;

        if xp_result.3 {
            self.send_prayer_level_up_updates(player_id, xp_result.2)
                .await;
        }
    }

    pub async fn handle_offer_all_bones(&self, player_id: &str, item_id: &str, altar_id: &str) {
        tracing::debug!(
            "Player {} offering all {} at altar {}",
            player_id,
            item_id,
            altar_id
        );

        if !self.validate_altar_access(player_id, altar_id).await {
            return;
        }

        let (item_name, base_prayer_xp) = match self.item_registry.get(item_id) {
            Some(def) if def.is_bones() => (def.display_name.clone(), def.prayer_xp),
            _ => {
                self.send_system_message(player_id, "You can only offer bones at the altar.")
                    .await;
                return;
            }
        };

        let altar_xp_per = altar_xp_for_item(item_id, base_prayer_xp);

        let (total_bones, inv_msg, xp_result) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            let mut total = 0i32;
            for slot in &mut player.inventory.slots {
                if slot.as_ref().is_some_and(|s| s.item_id == item_id) {
                    total += slot.as_ref().unwrap().quantity;
                    *slot = None;
                }
            }

            if total == 0 {
                drop(players);
                self.send_system_message(player_id, "You don't have any of those bones.")
                    .await;
                return;
            }

            let total_xp = altar_xp_per as i64 * total as i64;
            let leveled = player.skills.prayer.add_xp(total_xp);
            if leveled {
                player.prayer_points = player.max_prayer_points();
            }

            (
                total,
                inventory_update_message(player_id, player),
                (
                    total_xp,
                    player.skills.prayer.xp,
                    player.skills.prayer.level,
                    leveled,
                ),
            )
        };

        self.send_system_message(
            player_id,
            &format!(
                "The gods are pleased with your offering of {} {}. (+{} Prayer XP)",
                total_bones, item_name, xp_result.0
            ),
        )
        .await;
        self.send_to_player(player_id, inv_msg).await;
        self.send_to_player(
            player_id,
            prayer_xp_message(player_id, xp_result.0, xp_result.1, xp_result.2),
        )
        .await;

        if xp_result.3 {
            self.send_prayer_level_up_updates(player_id, xp_result.2)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn test_prayer_registry() -> PrayerRegistry {
        let temp_dir = TempDir::new().unwrap();
        let prayers_file = temp_dir.path().join("prayers.toml");
        let toml = r#"
[clarity]
name = "Clarity"
level_req = 1
category = "attack"
effect_type = "attack_bonus"
effect_value = 5.0
drain_rate = 1.0

[improved_clarity]
name = "Improved Clarity"
level_req = 10
category = "attack"
effect_type = "attack_bonus"
effect_value = 10.0
drain_rate = 2.0

[thick_skin]
name = "Thick Skin"
level_req = 1
category = "defence"
effect_type = "defence_bonus"
effect_value = 5.0
drain_rate = 1.0
"#;

        let mut file = std::fs::File::create(&prayers_file).unwrap();
        file.write_all(toml.as_bytes()).unwrap();

        let mut registry = PrayerRegistry::new();
        registry.load_from_file(&prayers_file).unwrap();
        registry
    }

    #[test]
    fn altar_xp_for_item_uses_known_overrides_and_fallback_multiplier() {
        assert_eq!(altar_xp_for_item("regular_bones", 5), 12);
        assert_eq!(altar_xp_for_item("big_bones", 15), 37);
        assert_eq!(altar_xp_for_item("dragon_bones", 72), 180);
        assert_eq!(altar_xp_for_item("wyrm_bones", 40), 100);
    }

    #[test]
    fn apply_prayer_toggle_replaces_same_category_and_deactivates_on_repeat() {
        let registry = test_prayer_registry();
        let mut active = HashSet::from(["clarity".to_string(), "thick_skin".to_string()]);

        let activated = apply_prayer_toggle(
            &mut active,
            "improved_clarity",
            crate::prayer::PrayerCategory::Attack,
            &registry,
        );

        assert_eq!(
            activated,
            PrayerToggleAction::Activated {
                replaced: Some("clarity".to_string())
            }
        );
        assert!(active.contains("improved_clarity"));
        assert!(active.contains("thick_skin"));
        assert!(!active.contains("clarity"));

        let deactivated = apply_prayer_toggle(
            &mut active,
            "improved_clarity",
            crate::prayer::PrayerCategory::Attack,
            &registry,
        );

        assert_eq!(deactivated, PrayerToggleAction::Deactivated);
        assert!(!active.contains("improved_clarity"));
        assert!(active.contains("thick_skin"));
    }

    #[test]
    fn altar_dialogue_text_reflects_prayer_restore_availability() {
        let restore_prompt = altar_dialogue_text(8, 15);
        assert!(restore_prompt.contains("Would you like to pray"));
        assert!(restore_prompt.contains("8/15"));

        let full_prompt = altar_dialogue_text(15, 15);
        assert!(full_prompt.contains("already full"));
        assert!(full_prompt.contains("15/15"));
    }
}
