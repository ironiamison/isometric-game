use super::GameRoom;
use crate::protocol::ServerMessage;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct OrderTemplate {
    pub id: String,
    pub tier: String,
    pub skill: String,
    pub min_level: i32,
    pub items: Vec<OrderItem>,
    pub rewards: OrderRewards,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderItem {
    pub id: String,
    pub quantity: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderRewards {
    pub gold: i32,
    pub marks: i32,
    pub xp: HashMap<String, i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct OrdersFile {
    orders: Vec<OrderTemplate>,
}

// ============================================================================
// Registry
// ============================================================================

pub struct CraftingOrderRegistry {
    orders: Vec<OrderTemplate>,
}

impl CraftingOrderRegistry {
    /// Load all order templates from data/orders/*.toml
    pub fn load(data_path: &str) -> Self {
        let orders_dir = format!("{}/orders", data_path);
        let mut orders = Vec::new();

        let entries = match std::fs::read_dir(&orders_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read orders directory {}: {}", orders_dir, e);
                return Self { orders };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<OrdersFile>(&content) {
                    Ok(file) => {
                        tracing::info!(
                            "Loaded {} orders from {}",
                            file.orders.len(),
                            path.display()
                        );
                        orders.extend(file.orders);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read {}: {}", path.display(), e);
                }
            }
        }

        tracing::info!("Loaded {} crafting order templates total", orders.len());
        Self { orders }
    }

    /// Get an order by ID
    pub fn get_order(&self, id: &str) -> Option<&OrderTemplate> {
        self.orders.iter().find(|o| o.id == id)
    }

    /// Generate daily orders for a player based on their skill levels.
    /// Returns 3-5 order IDs, with at least 1 regular and 1 masterwork (if eligible).
    pub fn generate_daily_orders(&self, player_skills: &HashMap<String, i32>) -> Vec<String> {
        let mut rng = rand::thread_rng();

        // Filter orders where player has the required skill level
        let eligible: Vec<&OrderTemplate> = self
            .orders
            .iter()
            .filter(|o| {
                player_skills
                    .get(&o.skill)
                    .copied()
                    .unwrap_or(1)
                    >= o.min_level
            })
            .collect();

        if eligible.is_empty() {
            return Vec::new();
        }

        // Check if player has 40+ in any skill (masterwork eligibility)
        let has_masterwork_eligible_skill = player_skills.values().any(|&level| level >= 40);

        // Separate into regular and masterwork pools
        let regular: Vec<&OrderTemplate> =
            eligible.iter().filter(|o| o.tier == "regular").copied().collect();
        let masterwork: Vec<&OrderTemplate> = eligible
            .iter()
            .filter(|o| o.tier == "masterwork")
            .copied()
            .collect();

        let total_count = rng.gen_range(3..=5);
        let mut selected: Vec<String> = Vec::new();

        // Pick at least 1 regular (if any available)
        if let Some(order) = regular.choose(&mut rng) {
            selected.push(order.id.clone());
        }

        // Pick at least 1 masterwork (if eligible and available)
        if has_masterwork_eligible_skill {
            if let Some(order) = masterwork.choose(&mut rng) {
                if !selected.contains(&order.id) {
                    selected.push(order.id.clone());
                }
            }
        }

        // Fill remaining slots from both pools
        let mut combined = eligible.clone();
        combined.shuffle(&mut rng);

        for order in combined {
            if selected.len() >= total_count {
                break;
            }
            if !selected.contains(&order.id) {
                selected.push(order.id.clone());
            }
        }

        selected
    }
}

// ============================================================================
// Helper: skill level lookup from Skills struct by string name
// ============================================================================

fn skill_level_by_name(skills: &crate::skills::Skills, name: &str) -> i32 {
    match name {
        "smithing" => skills.smithing.level,
        "mining" => skills.mining.level,
        "woodcutting" => skills.woodcutting.level,
        "fishing" => skills.fishing.level,
        "farming" => skills.farming.level,
        "alchemy" => skills.alchemy.level,
        "attack" => skills.attack.level,
        "strength" => skills.strength.level,
        "defence" => skills.defence.level,
        "ranged" => skills.ranged.level,
        "magic" => skills.magic.level,
        "prayer" => skills.prayer.level,
        "hitpoints" => skills.hitpoints.level,
        "slayer" => skills.slayer.level,
        "survivalist" => skills.survivalist.level,
        _ => 1,
    }
}

fn add_xp_to_skill(
    skills: &mut crate::skills::Skills,
    name: &str,
    amount: i64,
) -> Option<(bool, i64, i32)> {
    let skill = match name {
        "smithing" => &mut skills.smithing,
        "mining" => &mut skills.mining,
        "woodcutting" => &mut skills.woodcutting,
        "fishing" => &mut skills.fishing,
        "farming" => &mut skills.farming,
        "alchemy" => &mut skills.alchemy,
        "attack" => &mut skills.attack,
        "strength" => &mut skills.strength,
        "defence" => &mut skills.defence,
        "ranged" => &mut skills.ranged,
        "magic" => &mut skills.magic,
        "prayer" => &mut skills.prayer,
        "hitpoints" => &mut skills.hitpoints,
        "slayer" => &mut skills.slayer,
        "survivalist" => &mut skills.survivalist,
        _ => return None,
    };
    let leveled_up = skill.add_xp(amount);
    Some((leveled_up, skill.xp, skill.level))
}

/// Build a HashMap of skill name -> level from the Skills struct.
pub fn skills_to_map(skills: &crate::skills::Skills) -> HashMap<String, i32> {
    let mut map = HashMap::new();
    map.insert("smithing".to_string(), skills.smithing.level);
    map.insert("mining".to_string(), skills.mining.level);
    map.insert("woodcutting".to_string(), skills.woodcutting.level);
    map.insert("fishing".to_string(), skills.fishing.level);
    map.insert("farming".to_string(), skills.farming.level);
    map.insert("alchemy".to_string(), skills.alchemy.level);
    map.insert("attack".to_string(), skills.attack.level);
    map.insert("strength".to_string(), skills.strength.level);
    map.insert("defence".to_string(), skills.defence.level);
    map.insert("ranged".to_string(), skills.ranged.level);
    map.insert("magic".to_string(), skills.magic.level);
    map.insert("prayer".to_string(), skills.prayer.level);
    map.insert("hitpoints".to_string(), skills.hitpoints.level);
    map.insert("slayer".to_string(), skills.slayer.level);
    map.insert("survivalist".to_string(), skills.survivalist.level);
    map
}

// ============================================================================
// GameRoom Handlers
// ============================================================================

impl GameRoom {
    /// Accept a crafting order.
    pub(in crate::game) async fn handle_accept_crafting_order(
        &self,
        player_id: &str,
        order_id: &str,
    ) {
        // 1. Check player doesn't already have an active order
        if let Some(ref db) = self.db {
            let Some(character_id) = Self::parse_character_id(player_id) else {
                self.send_system_message(player_id, "Could not resolve character.")
                    .await;
                return;
            };

            match db.get_active_order(character_id).await {
                Ok(Some(_)) => {
                    self.send_system_message(
                        player_id,
                        "You already have an active crafting order. Complete or abandon it first.",
                    )
                    .await;
                    return;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!("Failed to check active crafting order: {}", e);
                    self.send_system_message(player_id, "Something went wrong. Try again.")
                        .await;
                    return;
                }
            }

            // 2. Get the order template from registry
            let Some(template) = self.crafting_order_registry.get_order(order_id) else {
                self.send_system_message(player_id, "Unknown crafting order.")
                    .await;
                return;
            };

            // 3. Validate player's skill level meets min_level
            let player_level = {
                let players = self.players.read().await;
                let Some(player) = players.get(player_id) else {
                    return;
                };
                skill_level_by_name(&player.skills, &template.skill)
            };

            if player_level < template.min_level {
                self.send_system_message(
                    player_id,
                    &format!(
                        "You need {} level {} for this order.",
                        template.skill, template.min_level
                    ),
                )
                .await;
                return;
            }

            // 4. Save to DB
            if let Err(e) = db.save_active_order(character_id, order_id).await {
                tracing::error!("Failed to save active crafting order: {}", e);
                self.send_system_message(player_id, "Something went wrong. Try again.")
                    .await;
                return;
            }

            // 5. Send system message confirming acceptance
            let item_summary: Vec<String> = template
                .items
                .iter()
                .map(|item| {
                    let name = self
                        .item_registry
                        .get(&item.id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| item.id.clone());
                    format!("{}x {}", item.quantity, name)
                })
                .collect();

            self.send_system_message(
                player_id,
                &format!(
                    "Crafting order accepted! Deliver: {}.",
                    item_summary.join(", ")
                ),
            )
            .await;
        } else {
            self.send_system_message(player_id, "Database not available.")
                .await;
        }
    }

    /// Claim a completed crafting order.
    pub(in crate::game) async fn handle_claim_crafting_order(&self, player_id: &str) {
        let Some(ref db) = self.db else {
            self.send_system_message(player_id, "Database not available.")
                .await;
            return;
        };
        let Some(character_id) = Self::parse_character_id(player_id) else {
            self.send_system_message(player_id, "Could not resolve character.")
                .await;
            return;
        };

        // 1. Get active order from DB
        let active_order_id = match db.get_active_order(character_id).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                self.send_system_message(player_id, "You don't have an active crafting order.")
                    .await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to get active crafting order: {}", e);
                self.send_system_message(player_id, "Something went wrong. Try again.")
                    .await;
                return;
            }
        };

        // 2. Get order template from registry
        let Some(template) = self.crafting_order_registry.get_order(&active_order_id) else {
            tracing::error!(
                "Active order {} not found in registry for player {}",
                active_order_id,
                player_id
            );
            self.send_system_message(player_id, "Order template not found. Try abandoning.")
                .await;
            return;
        };
        let template = template.clone();

        // 3-6. Check inventory, remove items, grant rewards (all under player write lock)
        let result = {
            let mut players = self.players.write().await;
            let Some(player) = players.get_mut(player_id) else {
                return;
            };

            // 3. Check player's inventory has all required items
            for item in &template.items {
                if !player.inventory.has_item(&item.id, item.quantity) {
                    let name = self
                        .item_registry
                        .get(&item.id)
                        .map(|def| def.display_name.clone())
                        .unwrap_or_else(|| item.id.clone());
                    let have = player.inventory.count_item(&item.id);
                    return self
                        .send_system_message(
                            player_id,
                            &format!(
                                "You need {}x {} ({}/{}).",
                                item.quantity, name, have, item.quantity
                            ),
                        )
                        .await;
                }
            }

            // 4. Remove items from inventory
            for item in &template.items {
                player.inventory.remove_item(&item.id, item.quantity);
            }

            // 5. Grant gold
            player.inventory.gold += template.rewards.gold;

            // 6. Grant XP for each skill in the rewards
            let mut xp_results: Vec<(String, i64, i64, i32, bool)> = Vec::new();
            for (skill_name, &xp_amount) in &template.rewards.xp {
                if let Some((leveled_up, total_xp, level)) =
                    add_xp_to_skill(&mut player.skills, skill_name, xp_amount)
                {
                    xp_results.push((
                        skill_name.clone(),
                        xp_amount,
                        total_xp,
                        level,
                        leveled_up,
                    ));
                }
            }

            let inventory_update = player.inventory.to_update();
            let gold = player.inventory.gold;

            Some((inventory_update, gold, xp_results))
        };

        let Some((inventory_update, gold, xp_results)) = result else {
            return;
        };

        // 7. Grant commission marks if marks > 0
        if template.rewards.marks > 0 {
            if let Err(e) = db
                .add_commission_marks(character_id, template.rewards.marks)
                .await
            {
                tracing::warn!("Failed to add commission marks: {}", e);
            }
        }

        // 8. Increment crafting order stats
        let is_masterwork = template.tier == "masterwork";
        if let Err(e) = db
            .increment_crafting_order_stats(character_id, is_masterwork, template.rewards.marks)
            .await
        {
            tracing::warn!("Failed to increment crafting order stats: {}", e);
        }

        // 9. Remove active order from DB
        if let Err(e) = db.remove_active_order(character_id).await {
            tracing::error!("Failed to remove active crafting order: {}", e);
        }

        // Send inventory update
        self.send_to_player(
            player_id,
            ServerMessage::InventoryUpdate {
                player_id: player_id.to_string(),
                slots: inventory_update,
                gold,
            },
        )
        .await;

        // Send XP updates and handle level-ups
        for (skill_name, xp_gained, total_xp, level, leveled_up) in &xp_results {
            self.send_to_player(
                player_id,
                ServerMessage::SkillXp {
                    player_id: player_id.to_string(),
                    skill: skill_name.clone(),
                    xp_gained: *xp_gained,
                    total_xp: *total_xp,
                    level: *level,
                },
            )
            .await;

            if *leveled_up {
                self.broadcast_skill_level_up(player_id, skill_name, *level)
                    .await;
                self.process_quest_progression_snapshot(player_id).await;
            }
        }

        // 10. Send system message with rewards
        let xp_text: Vec<String> = xp_results
            .iter()
            .map(|(skill, xp, _, _, _)| format!("{} {} XP", xp, skill))
            .collect();
        let marks_text = if template.rewards.marks > 0 {
            format!(", {} commission mark(s)", template.rewards.marks)
        } else {
            String::new()
        };

        self.send_system_message(
            player_id,
            &format!(
                "Crafting order complete! Received {}, {}gp{}.",
                xp_text.join(", "),
                template.rewards.gold,
                marks_text
            ),
        )
        .await;
    }

    /// Abandon active crafting order.
    pub(in crate::game) async fn handle_abandon_crafting_order(&self, player_id: &str) {
        let Some(ref db) = self.db else {
            self.send_system_message(player_id, "Database not available.")
                .await;
            return;
        };
        let Some(character_id) = Self::parse_character_id(player_id) else {
            self.send_system_message(player_id, "Could not resolve character.")
                .await;
            return;
        };

        // Check they actually have an active order
        match db.get_active_order(character_id).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                self.send_system_message(player_id, "You don't have an active crafting order.")
                    .await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to check active crafting order: {}", e);
                self.send_system_message(player_id, "Something went wrong. Try again.")
                    .await;
                return;
            }
        }

        // 1. Remove active order from DB
        if let Err(e) = db.remove_active_order(character_id).await {
            tracing::error!("Failed to remove active crafting order: {}", e);
            self.send_system_message(player_id, "Something went wrong. Try again.")
                .await;
            return;
        }

        // 2. Send system message confirming abandonment
        self.send_system_message(player_id, "Crafting order abandoned.")
            .await;
    }
}
