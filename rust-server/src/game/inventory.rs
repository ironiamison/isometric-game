use super::GameRoom;
use crate::data::item_def::{EquipmentSlot, UseEffect, WeaponType};
use crate::item::{self, GOLD_ITEM_ID, GroundItem, InventorySlotUpdate};
use crate::protocol::ServerMessage;

type EquipmentState = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemUseRoute {
    RecipeScroll,
    SpellScroll,
    DigTool,
    Consumable,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn classify_item_use(item_id: &str, use_effect: Option<&UseEffect>) -> ItemUseRoute {
    if item_id.starts_with("recipe_") {
        ItemUseRoute::RecipeScroll
    } else if matches!(use_effect, Some(UseEffect::LearnSpell { .. })) {
        ItemUseRoute::SpellScroll
    } else if matches!(use_effect, Some(UseEffect::Dig)) {
        ItemUseRoute::DigTool
    } else {
        ItemUseRoute::Consumable
    }
}

fn resolve_drop_position(
    player_x: i32,
    player_y: i32,
    target_x: Option<i32>,
    target_y: Option<i32>,
) -> (i32, i32) {
    if let (Some(target_x), Some(target_y)) = (target_x, target_y) {
        let dx = (target_x - player_x).abs();
        let dy = (target_y - player_y).abs();
        if dx <= 1 && dy <= 1 {
            return (target_x, target_y);
        }
    }

    (player_x, player_y)
}

fn parse_equipment_slot(slot_type: &str) -> Option<EquipmentSlot> {
    match slot_type {
        "head" => Some(EquipmentSlot::Head),
        "body" => Some(EquipmentSlot::Body),
        "weapon" => Some(EquipmentSlot::Weapon),
        "back" => Some(EquipmentSlot::Back),
        "feet" => Some(EquipmentSlot::Feet),
        "ring" => Some(EquipmentSlot::Ring),
        "gloves" => Some(EquipmentSlot::Gloves),
        "necklace" => Some(EquipmentSlot::Necklace),
        "belt" => Some(EquipmentSlot::Belt),
        _ => None,
    }
}

fn should_stop_gathering_for_weapon(item_id: &str) -> bool {
    !matches!(item_id, "fishing_rod" | "maple_rod")
}

fn inventory_update_message(
    player_id: &str,
    slots: Vec<InventorySlotUpdate>,
    gold: i32,
) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        player_id: player_id.to_string(),
        slots,
        gold,
    }
}

fn equipment_update_message(player_id: &str, equipment: EquipmentState) -> ServerMessage {
    ServerMessage::EquipmentUpdate {
        player_id: player_id.to_string(),
        equipped_head: equipment.0,
        equipped_body: equipment.1,
        equipped_weapon: equipment.2,
        equipped_back: equipment.3,
        equipped_feet: equipment.4,
        equipped_ring: equipment.5,
        equipped_gloves: equipment.6,
        equipped_necklace: equipment.7,
        equipped_belt: equipment.8,
    }
}

impl GameRoom {
    pub async fn handle_use_item(&self, player_id: &str, slot_index: u8) {
        if self.is_slot_in_trade(player_id, slot_index).await {
            self.send_system_message(player_id, "That item is in a trade offer.")
                .await;
            return;
        }

        {
            let arena = self.arena_manager.read().await;
            if arena.is_fighting() && arena.is_in_ring(player_id) {
                self.send_system_message(player_id, "You can't use items during an arena fight!")
                    .await;
                return;
            }
        }

        {
            let players = self.players.read().await;
            if let Some(player) = players.get(player_id) {
                if player.is_dead {
                    return;
                }
                if let Some(slot) = player
                    .inventory
                    .slots
                    .get(slot_index as usize)
                    .and_then(|slot| slot.as_ref())
                {
                    let route = classify_item_use(
                        &slot.item_id,
                        self.item_registry
                            .get(&slot.item_id)
                            .and_then(|definition| definition.use_effect.as_ref()),
                    );

                    drop(players);
                    match route {
                        ItemUseRoute::RecipeScroll => {
                            self.handle_use_recipe_scroll(player_id, slot_index).await;
                            return;
                        }
                        ItemUseRoute::SpellScroll => {
                            self.handle_use_spell_scroll(player_id, slot_index).await;
                            return;
                        }
                        ItemUseRoute::DigTool => {
                            self.handle_dig(player_id).await;
                            return;
                        }
                        ItemUseRoute::Consumable => {}
                    }
                }
            }
        }

        let (used_item_id, effect, inventory_update, gold, prayer_state, teleport_pos) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if player.is_dead {
                    return;
                }

                if let Some(item_id) = player
                    .inventory
                    .use_item(slot_index as usize, &self.item_registry)
                {
                    let mut prayer_state = None;
                    let mut teleport_pos: Option<(i32, i32)> = None;
                    let effect = if let Some(definition) = self.item_registry.get(&item_id) {
                        match &definition.use_effect {
                            Some(UseEffect::Heal { amount }) => {
                                player.hp = (player.hp + amount).min(player.max_hp());
                                format!("heal:{}", amount)
                            }
                            Some(UseEffect::RestoreMana { amount }) => {
                                let max_mp = player.max_mp();
                                let old_mp = player.mp;
                                player.mp = (player.mp + amount).min(max_mp);
                                let restored = player.mp - old_mp;
                                format!("mana:{}", restored)
                            }
                            Some(UseEffect::RestorePrayer { amount }) => {
                                let restore_amount = amount + (player.skills.prayer.level / 4);
                                let old_points = player.prayer_points;
                                player.prayer_points = (player.prayer_points + restore_amount)
                                    .min(player.max_prayer_points());
                                let restored = player.prayer_points - old_points;
                                prayer_state = Some(ServerMessage::PrayerStateUpdate {
                                    points: player.prayer_points,
                                    max_points: player.max_prayer_points(),
                                    active_prayers: player
                                        .active_prayers
                                        .iter()
                                        .cloned()
                                        .collect(),
                                });
                                format!("prayer:{}", restored)
                            }
                            Some(UseEffect::Buff {
                                stat,
                                amount,
                                duration_ms,
                            }) => {
                                player.apply_buff(stat.clone(), *amount, *duration_ms, now_ms(), item_id.clone());
                                format!("buff:{}:{}:{}", stat, amount, duration_ms)
                            }
                            Some(UseEffect::Teleport { destination, x, y }) => {
                                player.x = *x;
                                player.y = *y;
                                player.move_dx = 0;
                                player.move_dy = 0;
                                teleport_pos = Some((*x, *y));
                                format!("teleport:{}", destination)
                            }
                            Some(UseEffect::LearnSpell { .. }) | Some(UseEffect::Dig) | None => {
                                "none".to_string()
                            }
                        }
                    } else {
                        "none".to_string()
                    };

                    (
                        Some(item_id),
                        effect,
                        player.inventory.to_update(),
                        player.inventory.gold,
                        prayer_state,
                        teleport_pos,
                    )
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        if let Some(item_id) = used_item_id {
            let display_name = self
                .item_registry
                .get(&item_id)
                .map(|definition| definition.display_name.as_str())
                .unwrap_or(&item_id);
            tracing::debug!("Player {} used {} ({})", player_id, display_name, effect);

            self.send_to_player(
                player_id,
                ServerMessage::ItemUsed {
                    player_id: player_id.to_string(),
                    slot: slot_index,
                    item_id: item_id.clone(),
                    effect,
                },
            )
            .await;

            self.send_to_player(
                player_id,
                inventory_update_message(player_id, inventory_update, gold),
            )
            .await;

            if let Some(prayer_update) = prayer_state {
                self.send_to_player(player_id, prayer_update).await;
            }

            // Post-teleport: preload chunks and handle instance exit
            if let Some((tx, ty)) = teleport_pos {
                use crate::chunk::ChunkCoord;

                // If player was in an instance, exit them to overworld
                let was_in_instance = self.player_instances.write().await.remove(player_id);
                if let Some(instance_id) = was_in_instance {
                    self.reset_sync_state(player_id).await;
                    if let Some(instance) = self.instance_manager.get_by_instance_id(&instance_id) {
                        let other_players: Vec<String> = instance
                            .get_player_ids()
                            .await
                            .into_iter()
                            .filter(|id| id != player_id)
                            .collect();

                        let remaining = instance.remove_player(player_id).await;
                        if remaining == 0
                            && instance.instance_type == crate::interior::InstanceType::Private
                        {
                            if let Some(owner_id) = &instance.owner_id {
                                self.instance_manager
                                    .remove_private(owner_id, &instance.map_id);
                            }
                        }

                        for other_id in &other_players {
                            self.send_to_player(
                                other_id,
                                ServerMessage::PlayerLeft {
                                    id: player_id.to_string(),
                                },
                            )
                            .await;
                            self.send_to_player(
                                player_id,
                                ServerMessage::PlayerLeft {
                                    id: other_id.clone(),
                                },
                            )
                            .await;
                        }
                    }

                    self.send_to_player(
                        player_id,
                        ServerMessage::MapTransition {
                            map_type: "overworld".to_string(),
                            map_id: "world_0".to_string(),
                            spawn_x: tx as f32,
                            spawn_y: ty as f32,
                            instance_id: String::new(),
                        },
                    )
                    .await;
                }

                // Preload chunks around the teleport destination
                let spawn_chunk = ChunkCoord::from_world(tx, ty);
                self.world()
                    .preload_chunks(spawn_chunk, super::SPAWN_PRELOAD_RADIUS)
                    .await;
            }
        }
    }

    async fn handle_dig(&self, player_id: &str) {
        let (player_x, player_y) = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => (player.x, player.y),
                _ => return,
            }
        };

        let quest_state = {
            let quest_states = self.player_quest_states.read().await;
            quest_states.get(player_id).cloned()
        };

        let site_match = {
            let dig_sites = self.dig_site_manager.read().await;
            let mut found = None;
            for site in &dig_sites.sites {
                let dx = (player_x - site.x).abs();
                let dy = (player_y - site.y).abs();
                if dx > site.radius || dy > site.radius {
                    continue;
                }
                if dig_sites
                    .triggered
                    .contains(&(player_id.to_string(), site.id.clone()))
                {
                    continue;
                }
                if let Some(quest_state) = quest_state.as_ref() {
                    if let Some(progress) = quest_state.active_quests.get(&site.quest_id) {
                        if progress.status == crate::quest::QuestStatus::Active {
                            if let Some(objective) =
                                progress.objectives.get(&site.quest_objective_id)
                            {
                                if !objective.completed {
                                    found = Some(site.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            found
        };

        if let Some(site) = site_match {
            {
                let mut dig_sites = self.dig_site_manager.write().await;
                dig_sites.mark_triggered(player_id, &site.id);
            }

            self.send_system_message(
                player_id,
                "You dig into the ground... something is stirring beneath!",
            )
            .await;

            let dig_location_id = format!("{}_dig", site.id);
            self.process_quest_location_reached(player_id, &dig_location_id, site.x, site.y)
                .await;

            if let Some(prototype) = self.entity_registry.get(&site.spawn_entity) {
                let npc_id = format!("dig_{}_{}", site.id, player_id);
                let mut npc = crate::npc::Npc::from_prototype(
                    &npc_id,
                    &site.spawn_entity,
                    prototype,
                    site.x,
                    site.y,
                    site.spawn_level,
                    None,
                );
                npc.stats.respawn_time_ms = 0;
                let mut npcs = self.npcs.write().await;
                npcs.insert(npc_id, npc);
            }
        } else {
            self.send_system_message(player_id, "There's nothing to dig here.")
                .await;
        }
    }

    async fn handle_use_spell_scroll(&self, player_id: &str, slot_index: u8) {
        let (item_id, spell_id, inventory_update, gold) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            let item_id = match player
                .inventory
                .slots
                .get(slot_index as usize)
                .and_then(|slot| slot.as_ref())
            {
                Some(slot) => slot.item_id.clone(),
                None => return,
            };

            let spell_id = match self.item_registry.get(&item_id) {
                Some(definition) => match &definition.use_effect {
                    Some(UseEffect::LearnSpell { spell_id }) => spell_id.clone(),
                    _ => return,
                },
                None => return,
            };

            if self.scroll_spell_registry.get(&spell_id).is_none() {
                drop(players);
                self.send_system_message(player_id, "This scroll contains an unknown spell.")
                    .await;
                return;
            }

            if player.unlocked_spells.contains(&spell_id) {
                drop(players);
                self.send_system_message(player_id, "You already know this spell.")
                    .await;
                return;
            }

            if let Some(slot) = player.inventory.slots[slot_index as usize].as_mut() {
                slot.quantity -= 1;
                if slot.quantity <= 0 {
                    player.inventory.slots[slot_index as usize] = None;
                }
            }

            player.unlocked_spells.insert(spell_id.clone());

            (
                item_id,
                spell_id,
                player.inventory.to_update(),
                player.inventory.gold,
            )
        };

        let spell_name = self
            .scroll_spell_registry
            .get(&spell_id)
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| spell_id.clone());
        tracing::info!(
            "Player {} used spell scroll {} -> unlocked spell {} ({})",
            player_id,
            item_id,
            spell_id,
            spell_name
        );

        if let Some(db) = self.db.as_ref() {
            if let Some(character_id) = Self::parse_character_id(player_id) {
                if let Err(error) = db.save_unlocked_spell(character_id, &spell_id).await {
                    tracing::warn!("Failed to save unlocked spell to DB: {}", error);
                }
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::SpellUnlocked {
                spell_id: spell_id.clone(),
            },
        )
        .await;

        self.send_system_message(player_id, &format!("You have learned {}!", spell_name))
            .await;

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;
    }

    pub async fn handle_equip(&self, player_id: &str, slot_index: u8) {
        let slot_index_usize = slot_index as usize;

        if self.is_slot_in_trade(player_id, slot_index).await {
            self.send_system_message(player_id, "That item is in a trade offer.")
                .await;
            return;
        }

        let item_info = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            match player.inventory.slots.get(slot_index_usize) {
                Some(Some(slot)) => Some((
                    slot.item_id.clone(),
                    player.skills.attack.level,
                    player.skills.defence.level,
                    player.skills.woodcutting.level,
                    player.skills.magic.level,
                    player.skills.ranged.level,
                    (
                        player.equipped_head.clone(),
                        player.equipped_body.clone(),
                        player.equipped_weapon.clone(),
                        player.equipped_back.clone(),
                        player.equipped_feet.clone(),
                        player.equipped_ring.clone(),
                        player.equipped_gloves.clone(),
                        player.equipped_necklace.clone(),
                        player.equipped_belt.clone(),
                    ),
                )),
                _ => None,
            }
        };

        let (item_id, attack_level, defence_level, woodcutting_level, magic_level, ranged_level, current_equipment) =
            match item_info {
                Some(info) => info,
                None => {
                    self.send_to_player(
                        player_id,
                        ServerMessage::EquipResult {
                            success: false,
                            slot_type: "unknown".to_string(),
                            item_id: None,
                            error: Some("No item in that slot".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            };

        let item_def = match self.item_registry.get(&item_id) {
            Some(definition) => definition,
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::EquipResult {
                        success: false,
                        slot_type: "unknown".to_string(),
                        item_id: None,
                        error: Some("Item not found".to_string()),
                    },
                )
                .await;
                return;
            }
        };

        let (equip_stats, equip_slot) = match &item_def.equipment {
            Some(stats) if stats.slot_type != EquipmentSlot::None => (stats, stats.slot_type),
            _ => {
                self.send_to_player(
                    player_id,
                    ServerMessage::EquipResult {
                        success: false,
                        slot_type: "unknown".to_string(),
                        item_id: None,
                        error: Some("Item cannot be equipped".to_string()),
                    },
                )
                .await;
                return;
            }
        };

        let slot_type = equip_slot.as_str().to_string();
        if equip_stats.attack_level_required > 0
            && attack_level < equip_stats.attack_level_required
        {
            self.send_to_player(
                player_id,
                ServerMessage::EquipResult {
                    success: false,
                    slot_type,
                    item_id: None,
                    error: Some(format!(
                        "Requires Attack level {}",
                        equip_stats.attack_level_required
                    )),
                },
            )
            .await;
            return;
        }
        if equip_stats.defence_level_required > 0
            && defence_level < equip_stats.defence_level_required
        {
            self.send_to_player(
                player_id,
                ServerMessage::EquipResult {
                    success: false,
                    slot_type,
                    item_id: None,
                    error: Some(format!(
                        "Requires Defence level {}",
                        equip_stats.defence_level_required
                    )),
                },
            )
            .await;
            return;
        }

        if equip_stats.woodcutting_level_required > 0
            && woodcutting_level < equip_stats.woodcutting_level_required
        {
            self.send_to_player(
                player_id,
                ServerMessage::EquipResult {
                    success: false,
                    slot_type,
                    item_id: None,
                    error: Some(format!(
                        "Requires Woodcutting level {}",
                        equip_stats.woodcutting_level_required
                    )),
                },
            )
            .await;
            return;
        }

        if equip_stats.magic_level_required > 0 && magic_level < equip_stats.magic_level_required {
            self.send_to_player(
                player_id,
                ServerMessage::EquipResult {
                    success: false,
                    slot_type,
                    item_id: None,
                    error: Some(format!(
                        "Requires Magic level {}",
                        equip_stats.magic_level_required
                    )),
                },
            )
            .await;
            return;
        }

        if equip_stats.ranged_level_required > 0
            && ranged_level < equip_stats.ranged_level_required
        {
            self.send_to_player(
                player_id,
                ServerMessage::EquipResult {
                    success: false,
                    slot_type,
                    item_id: None,
                    error: Some(format!(
                        "Requires Ranged level {}",
                        equip_stats.ranged_level_required
                    )),
                },
            )
            .await;
            return;
        }

        let currently_equipped = match equip_slot {
            EquipmentSlot::Head => current_equipment.0,
            EquipmentSlot::Body => current_equipment.1,
            EquipmentSlot::Weapon => current_equipment.2,
            EquipmentSlot::Back => current_equipment.3,
            EquipmentSlot::Feet => current_equipment.4,
            EquipmentSlot::Ring => current_equipment.5,
            EquipmentSlot::Gloves => current_equipment.6,
            EquipmentSlot::Necklace => current_equipment.7,
            EquipmentSlot::Belt => current_equipment.8,
            EquipmentSlot::None => None,
        };

        let (inventory_update, gold, equipment) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            if let Some(old_item_id) = currently_equipped.as_ref() {
                player.inventory.slots[slot_index_usize] =
                    Some(item::InventorySlot::new(old_item_id.clone(), 1));
            } else {
                player.inventory.slots[slot_index_usize] = None;
            }

            match equip_slot {
                EquipmentSlot::Head => player.equipped_head = Some(item_id.clone()),
                EquipmentSlot::Body => player.equipped_body = Some(item_id.clone()),
                EquipmentSlot::Weapon => player.equipped_weapon = Some(item_id.clone()),
                EquipmentSlot::Back => player.equipped_back = Some(item_id.clone()),
                EquipmentSlot::Feet => player.equipped_feet = Some(item_id.clone()),
                EquipmentSlot::Ring => player.equipped_ring = Some(item_id.clone()),
                EquipmentSlot::Gloves => player.equipped_gloves = Some(item_id.clone()),
                EquipmentSlot::Necklace => player.equipped_necklace = Some(item_id.clone()),
                EquipmentSlot::Belt => player.equipped_belt = Some(item_id.clone()),
                EquipmentSlot::None => {}
            }

            (
                player.inventory.to_update(),
                player.inventory.gold,
                (
                    player.equipped_head.clone(),
                    player.equipped_body.clone(),
                    player.equipped_weapon.clone(),
                    player.equipped_back.clone(),
                    player.equipped_feet.clone(),
                    player.equipped_ring.clone(),
                    player.equipped_gloves.clone(),
                    player.equipped_necklace.clone(),
                    player.equipped_belt.clone(),
                ),
            )
        };

        tracing::info!(
            "Player {} equipped {} to {} slot",
            player_id,
            item_id,
            equip_slot.as_str()
        );

        if equip_slot == EquipmentSlot::Weapon && should_stop_gathering_for_weapon(&item_id) {
            let is_gathering = {
                let gathering = self.gathering.read().await;
                gathering.is_gathering(player_id)
            };
            if is_gathering {
                self.handle_stop_gathering(player_id).await;
            }
        }

        // Auto-fallback: if equipping a weapon, check if current combat style is valid
        if equip_slot == EquipmentSlot::Weapon {
            use crate::game::CombatStyle;
            let new_weapon_type = equip_stats.weapon_type;
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if !player.combat_style.is_valid_for(new_weapon_type) {
                    let available = CombatStyle::available_styles(new_weapon_type);
                    player.combat_style = available[0];
                    tracing::info!(
                        "Player {} combat style auto-reset to {} for {:?} weapon",
                        player_id,
                        player.combat_style.as_str(),
                        new_weapon_type
                    );
                }
            }
            drop(players);
        }

        self.send_to_player(
            player_id,
            ServerMessage::EquipResult {
                success: true,
                slot_type: equip_slot.as_str().to_string(),
                item_id: Some(item_id.clone()),
                error: None,
            },
        )
        .await;

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;

        self.broadcast(equipment_update_message(player_id, equipment))
            .await;
    }

    pub async fn handle_unequip(&self, player_id: &str, slot_type: &str) {
        let equip_slot = match parse_equipment_slot(slot_type) {
            Some(slot) => slot,
            None => {
                self.send_to_player(
                    player_id,
                    ServerMessage::EquipResult {
                        success: false,
                        slot_type: slot_type.to_string(),
                        item_id: None,
                        error: Some("Unknown equipment slot".to_string()),
                    },
                )
                .await;
                return;
            }
        };

        let equipped_item = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            let equipped_ref = match equip_slot {
                EquipmentSlot::Head => &player.equipped_head,
                EquipmentSlot::Body => &player.equipped_body,
                EquipmentSlot::Weapon => &player.equipped_weapon,
                EquipmentSlot::Back => &player.equipped_back,
                EquipmentSlot::Feet => &player.equipped_feet,
                EquipmentSlot::Ring => &player.equipped_ring,
                EquipmentSlot::Gloves => &player.equipped_gloves,
                EquipmentSlot::Necklace => &player.equipped_necklace,
                EquipmentSlot::Belt => &player.equipped_belt,
                EquipmentSlot::None => return,
            };

            match equipped_ref {
                Some(item_id) => {
                    if !player
                        .inventory
                        .has_space_for(item_id, 1, &self.item_registry)
                    {
                        self.send_to_player(
                            player_id,
                            ServerMessage::EquipResult {
                                success: false,
                                slot_type: slot_type.to_string(),
                                item_id: None,
                                error: Some("Inventory full".to_string()),
                            },
                        )
                        .await;
                        return;
                    }
                    Some(item_id.clone())
                }
                None => {
                    self.send_to_player(
                        player_id,
                        ServerMessage::EquipResult {
                            success: false,
                            slot_type: slot_type.to_string(),
                            item_id: None,
                            error: Some("Nothing equipped".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            }
        };

        let item_id = match equipped_item {
            Some(item_id) => item_id,
            None => return,
        };

        let (inventory_update, gold, equipment) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) => player,
                None => return,
            };

            match equip_slot {
                EquipmentSlot::Head => player.equipped_head = None,
                EquipmentSlot::Body => player.equipped_body = None,
                EquipmentSlot::Weapon => player.equipped_weapon = None,
                EquipmentSlot::Back => player.equipped_back = None,
                EquipmentSlot::Feet => player.equipped_feet = None,
                EquipmentSlot::Ring => player.equipped_ring = None,
                EquipmentSlot::Gloves => player.equipped_gloves = None,
                EquipmentSlot::Necklace => player.equipped_necklace = None,
                EquipmentSlot::Belt => player.equipped_belt = None,
                EquipmentSlot::None => {}
            }

            player.inventory.add_item(&item_id, 1, &self.item_registry);

            (
                player.inventory.to_update(),
                player.inventory.gold,
                (
                    player.equipped_head.clone(),
                    player.equipped_body.clone(),
                    player.equipped_weapon.clone(),
                    player.equipped_back.clone(),
                    player.equipped_feet.clone(),
                    player.equipped_ring.clone(),
                    player.equipped_gloves.clone(),
                    player.equipped_necklace.clone(),
                    player.equipped_belt.clone(),
                ),
            )
        };

        tracing::info!(
            "Player {} unequipped {} from {} slot",
            player_id,
            item_id,
            slot_type
        );

        if equip_slot == EquipmentSlot::Weapon {
            let is_gathering = {
                let gathering = self.gathering.read().await;
                gathering.is_gathering(player_id)
            };
            if is_gathering {
                self.handle_stop_gathering(player_id).await;
            }

            // Auto-fallback: unequipping weapon means unarmed (melee)
            use crate::game::CombatStyle;
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if !player.combat_style.is_valid_for(WeaponType::Melee) {
                    let available = CombatStyle::available_styles(WeaponType::Melee);
                    player.combat_style = available[0];
                }
            }
            drop(players);
        }

        self.send_to_player(
            player_id,
            ServerMessage::EquipResult {
                success: true,
                slot_type: slot_type.to_string(),
                item_id: Some(item_id),
                error: None,
            },
        )
        .await;

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;

        self.broadcast(equipment_update_message(player_id, equipment))
            .await;
    }

    pub async fn handle_drop_item(
        &self,
        player_id: &str,
        slot_index: u8,
        quantity: u32,
        target_x: Option<i32>,
        target_y: Option<i32>,
    ) {
        let slot_index_usize = slot_index as usize;

        if self.is_slot_in_trade(player_id, slot_index).await {
            self.send_system_message(player_id, "That item is in a trade offer.")
                .await;
            return;
        }

        let drop_info: Option<(i32, i32, String, i32)> = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            let (drop_x, drop_y) = resolve_drop_position(player.x, player.y, target_x, target_y);

            match player.inventory.slots.get(slot_index_usize) {
                Some(Some(slot)) => {
                    let quantity_to_drop = (quantity as i32).min(slot.quantity);
                    if quantity_to_drop <= 0 {
                        return;
                    }
                    Some((drop_x, drop_y, slot.item_id.clone(), quantity_to_drop))
                }
                _ => None,
            }
        };

        let (drop_x, drop_y, item_id, quantity_to_drop) = match drop_info {
            Some(info) => info,
            None => return,
        };

        let (inventory_update, gold) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(slot) = player.inventory.slots[slot_index_usize].as_mut() {
                    slot.quantity -= quantity_to_drop;
                    if slot.quantity <= 0 {
                        player.inventory.slots[slot_index_usize] = None;
                    }
                }
                (player.inventory.to_update(), player.inventory.gold)
            } else {
                return;
            }
        };

        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        let ground_item = GroundItem::new_in_instance(
            &uuid::Uuid::new_v4().to_string(),
            &item_id,
            drop_x as f32,
            drop_y as f32,
            quantity_to_drop,
            Some(player_id.to_string()),
            now_ms(),
            instance_id,
        );

        tracing::info!(
            "Player {} dropped {}x {} (protected for 10s)",
            player_id,
            quantity_to_drop,
            item_id
        );

        self.broadcast_to_zone(
            player_id,
            ServerMessage::ItemDropped {
                id: ground_item.id.clone(),
                item_id: item_id.clone(),
                x: drop_x as f32,
                y: drop_y as f32,
                quantity: quantity_to_drop,
            },
        )
        .await;

        {
            let mut ground_items = self.ground_items.write().await;
            ground_items.insert(ground_item.id.clone(), ground_item);
        }

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;
    }

    pub async fn handle_drop_gold(&self, player_id: &str, amount: i32) {
        if amount <= 0 {
            return;
        }

        let drop_info = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            if amount > player.inventory.gold {
                return;
            }

            Some((player.x, player.y))
        };

        let (player_x, player_y) = match drop_info {
            Some(info) => info,
            None => return,
        };

        let (inventory_update, gold) = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.inventory.gold -= amount;
                (player.inventory.to_update(), player.inventory.gold)
            } else {
                return;
            }
        };

        let instance_id = {
            let instances = self.player_instances.read().await;
            instances.get(player_id).cloned()
        };

        let ground_item = GroundItem::new_in_instance(
            &uuid::Uuid::new_v4().to_string(),
            GOLD_ITEM_ID,
            player_x as f32,
            player_y as f32,
            amount,
            Some(player_id.to_string()),
            now_ms(),
            instance_id,
        );

        tracing::info!(
            "Player {} dropped {}g (protected for 10s)",
            player_id,
            amount
        );

        self.broadcast_to_zone(
            player_id,
            ServerMessage::ItemDropped {
                id: ground_item.id.clone(),
                item_id: GOLD_ITEM_ID.to_string(),
                x: player_x as f32,
                y: player_y as f32,
                quantity: amount,
            },
        )
        .await;

        {
            let mut ground_items = self.ground_items.write().await;
            ground_items.insert(ground_item.id.clone(), ground_item);
        }

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;
    }

    pub async fn handle_swap_slots(&self, player_id: &str, from_slot: u8, to_slot: u8) {
        let from_index = from_slot as usize;
        let to_index = to_slot as usize;

        if from_index >= 20 || to_index >= 20 || from_index == to_index {
            return;
        }

        let (inventory_update, gold) = {
            let mut players = self.players.write().await;
            let player = match players.get_mut(player_id) {
                Some(player) if player.active && !player.is_dead => player,
                _ => return,
            };

            player.inventory.slots.swap(from_index, to_index);
            (player.inventory.to_update(), player.inventory.gold)
        };

        tracing::debug!(
            "Player {} swapped slots {} <-> {}",
            player_id,
            from_slot,
            to_slot
        );

        self.send_to_player(
            player_id,
            inventory_update_message(player_id, inventory_update, gold),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_item_use_prioritizes_recipe_and_use_effect_routes() {
        assert_eq!(
            classify_item_use("recipe_sword", Some(&UseEffect::Dig)),
            ItemUseRoute::RecipeScroll
        );
        assert_eq!(
            classify_item_use(
                "scroll_fire",
                Some(&UseEffect::LearnSpell {
                    spell_id: "fire".to_string()
                })
            ),
            ItemUseRoute::SpellScroll
        );
        assert_eq!(
            classify_item_use("spade", Some(&UseEffect::Dig)),
            ItemUseRoute::DigTool
        );
        assert_eq!(classify_item_use("potion", None), ItemUseRoute::Consumable);
    }

    #[test]
    fn resolve_drop_position_only_accepts_adjacent_targets() {
        assert_eq!(resolve_drop_position(10, 10, Some(11), Some(9)), (11, 9));
        assert_eq!(resolve_drop_position(10, 10, Some(12), Some(10)), (10, 10));
        assert_eq!(resolve_drop_position(10, 10, None, Some(10)), (10, 10));
    }

    #[test]
    fn parse_equipment_slot_and_gathering_weapon_rules_match_expected_values() {
        assert_eq!(parse_equipment_slot("weapon"), Some(EquipmentSlot::Weapon));
        assert_eq!(parse_equipment_slot("belt"), Some(EquipmentSlot::Belt));
        assert_eq!(parse_equipment_slot("invalid"), None);
        assert!(!should_stop_gathering_for_weapon("fishing_rod"));
        assert!(!should_stop_gathering_for_weapon("maple_rod"));
        assert!(should_stop_gathering_for_weapon("bronze_sword"));
    }
}
