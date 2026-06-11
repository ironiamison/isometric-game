use super::*;
use crate::game::titles::title_display;

pub struct PrestigeShopItem {
    pub id: &'static str,
    pub display: &'static str,
    pub cost: i32,
    pub item_type: PrestigeItemType,
}

pub enum PrestigeItemType {
    Title(&'static str),
    Equipment(&'static str),
}

pub const PRESTIGE_SHOP: &[PrestigeShopItem] = &[
    PrestigeShopItem {
        id: "buy_apprentice",
        display: "Apprentice Artisan Title",
        cost: 10,
        item_type: PrestigeItemType::Title("artisan_apprentice"),
    },
    PrestigeShopItem {
        id: "buy_master_smith",
        display: "Master Smith Title",
        cost: 30,
        item_type: PrestigeItemType::Title("master_smith"),
    },
    PrestigeShopItem {
        id: "buy_master_alchemist",
        display: "Master Alchemist Title",
        cost: 30,
        item_type: PrestigeItemType::Title("master_alchemist"),
    },
    PrestigeShopItem {
        id: "buy_master_fletcher",
        display: "Master Fletcher Title",
        cost: 30,
        item_type: PrestigeItemType::Title("master_fletcher"),
    },
    PrestigeShopItem {
        id: "buy_master_chef",
        display: "Master Chef Title",
        cost: 30,
        item_type: PrestigeItemType::Title("master_chef"),
    },
    PrestigeShopItem {
        id: "buy_grandmaster",
        display: "Grandmaster Artisan Title",
        cost: 100,
        item_type: PrestigeItemType::Title("grandmaster_artisan"),
    },
    PrestigeShopItem {
        id: "buy_cape",
        display: "Artisan's Cape",
        cost: 100,
        item_type: PrestigeItemType::Equipment("artisan_cape"),
    },
];

impl GameRoom {
    pub async fn show_prestige_shop_dialogue(&self, player_id: &str, npc_id: &str) {
        let db = match &self.db {
            Some(db) => db.clone(),
            None => return,
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        let marks = db.get_commission_marks(character_id).await.unwrap_or(0);
        let unlocked_titles = db.get_player_titles(character_id).await.unwrap_or_default();

        let mut text = format!(
            "Welcome, artisan! I trade in Commission Marks earned from masterwork orders.\n\nYour Commission Marks: {}",
            marks
        );

        let mut choices = Vec::new();

        for item in PRESTIGE_SHOP {
            let owned = match &item.item_type {
                PrestigeItemType::Title(title_id) => {
                    unlocked_titles.contains(&title_id.to_string())
                }
                PrestigeItemType::Equipment(_) => false,
            };

            if !owned {
                choices.push(crate::protocol::DialogueChoice {
                    id: item.id.to_string(),
                    text: format!("{} ({} marks)", item.display, item.cost),
                });
            }
        }

        choices.push(crate::protocol::DialogueChoice {
            id: "close".to_string(),
            text: "Close".to_string(),
        });

        self.send_to_player(
            player_id,
            ServerMessage::ShowDialogue {
                quest_id: format!("prestige_shop:{}", npc_id),
                npc_id: npc_id.to_string(),
                speaker: "Master Artisan".to_string(),
                text,
                choices,
            },
        )
        .await;
    }

    pub async fn handle_prestige_shop_choice(
        &self,
        player_id: &str,
        npc_id: &str,
        choice_id: &str,
    ) {
        if choice_id == "close" {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            return;
        }

        let db = match &self.db {
            Some(db) => db.clone(),
            None => return,
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            return;
        };

        // Find the shop item
        let Some(shop_item) = PRESTIGE_SHOP.iter().find(|i| i.id == choice_id) else {
            self.send_to_player(player_id, ServerMessage::DialogueClosed)
                .await;
            return;
        };

        // Check for titles: is it already owned?
        if let PrestigeItemType::Title(title_id) = &shop_item.item_type {
            let unlocked = db.get_player_titles(character_id).await.unwrap_or_default();
            if unlocked.contains(&title_id.to_string()) {
                self.send_system_message(player_id, "You already own that title.")
                    .await;
                self.show_prestige_shop_dialogue(player_id, npc_id).await;
                return;
            }
        }

        // Try to spend marks
        match db
            .spend_commission_marks(character_id, shop_item.cost)
            .await
        {
            Ok(true) => {
                // Purchase succeeded
                match &shop_item.item_type {
                    PrestigeItemType::Title(title_id) => {
                        if let Err(e) = db.unlock_title(character_id, title_id).await {
                            tracing::error!("Failed to unlock title {}: {}", title_id, e);
                            self.send_system_message(
                                player_id,
                                "Something went wrong unlocking the title.",
                            )
                            .await;
                        } else {
                            let display = title_display(title_id).unwrap_or(title_id);
                            self.send_system_message(
                                player_id,
                                &format!(
                                    "You unlocked the title: {}! Use /title set {} to equip it.",
                                    display, title_id
                                ),
                            )
                            .await;
                        }
                    }
                    PrestigeItemType::Equipment(item_id) => {
                        // Add item to inventory
                        let inv_msg = {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(player_id) {
                                player.inventory.add_item(item_id, 1, &self.item_registry);
                                Some(ServerMessage::InventoryUpdate {
                                    player_id: player_id.to_string(),
                                    slots: player.inventory.to_update(),
                                    gold: player.inventory.gold,
                                })
                            } else {
                                None
                            }
                        };
                        if let Some(msg) = inv_msg {
                            self.send_to_player(player_id, msg).await;
                        }
                        let display_name = self
                            .item_registry
                            .get(*item_id)
                            .map(|d| d.display_name.as_str())
                            .unwrap_or(item_id);
                        self.send_system_message(
                            player_id,
                            &format!("You received: {}!", display_name),
                        )
                        .await;
                    }
                }
            }
            Ok(false) => {
                self.send_system_message(
                    player_id,
                    &format!(
                        "You don't have enough Commission Marks. You need {} marks.",
                        shop_item.cost
                    ),
                )
                .await;
            }
            Err(e) => {
                tracing::error!("Failed to spend commission marks: {}", e);
                self.send_system_message(player_id, "Something went wrong. Try again.")
                    .await;
            }
        }

        // Re-show the dialogue with updated balance
        self.show_prestige_shop_dialogue(player_id, npc_id).await;
    }
}
