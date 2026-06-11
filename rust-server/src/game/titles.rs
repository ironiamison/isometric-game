use super::*;

pub struct TitleDef {
    pub id: &'static str,
    pub display: &'static str,
}

pub const TITLES: &[TitleDef] = &[
    // Crafting titles (purchased with Commission Marks)
    TitleDef {
        id: "artisan_apprentice",
        display: "Apprentice Artisan",
    },
    TitleDef {
        id: "master_smith",
        display: "Master Smith",
    },
    TitleDef {
        id: "master_alchemist",
        display: "Master Alchemist",
    },
    TitleDef {
        id: "master_fletcher",
        display: "Master Fletcher",
    },
    TitleDef {
        id: "master_chef",
        display: "Master Chef",
    },
    TitleDef {
        id: "grandmaster_artisan",
        display: "Grandmaster Artisan",
    },
    // Arena titles (unlocked by milestones)
    TitleDef {
        id: "arena_novice",
        display: "Brawler",
    },
    TitleDef {
        id: "arena_fighter",
        display: "Fighter",
    },
    TitleDef {
        id: "arena_veteran",
        display: "Veteran",
    },
    TitleDef {
        id: "arena_champion",
        display: "Champion",
    },
    TitleDef {
        id: "arena_legend",
        display: "Legend",
    },
];

pub fn title_display(title_id: &str) -> Option<&'static str> {
    TITLES.iter().find(|t| t.id == title_id).map(|t| t.display)
}

impl GameRoom {
    pub async fn handle_title_command(&self, player_id: &str, args: &[&str]) {
        let sub = args.first().map(|s| s.to_lowercase()).unwrap_or_default();

        let db = match &self.db {
            Some(db) => db.clone(),
            None => {
                self.send_system_message(player_id, "Database not available.")
                    .await;
                return;
            }
        };

        let Some(character_id) = Self::parse_character_id(player_id) else {
            self.send_system_message(player_id, "Could not resolve character.")
                .await;
            return;
        };

        match sub.as_str() {
            "list" => {
                let unlocked = db.get_player_titles(character_id).await.unwrap_or_default();
                if unlocked.is_empty() {
                    self.send_system_message(player_id, "You have no unlocked titles.")
                        .await;
                    return;
                }
                let list: Vec<String> = unlocked
                    .iter()
                    .map(|id| {
                        let display = title_display(id).unwrap_or(id.as_str());
                        format!("  {} ({})", display, id)
                    })
                    .collect();
                self.send_system_message(player_id, &format!("Your titles:\n{}", list.join("\n")))
                    .await;
            }
            "set" => {
                let Some(title_id) = args.get(1) else {
                    self.send_system_message(player_id, "Usage: /title set <title_id>")
                        .await;
                    return;
                };
                let title_id = title_id.to_lowercase();

                // Verify title exists
                let Some(display) = title_display(&title_id) else {
                    self.send_system_message(
                        player_id,
                        &format!(
                            "Unknown title: {}. Use /title list to see your titles.",
                            title_id
                        ),
                    )
                    .await;
                    return;
                };

                // Verify player has unlocked it
                let unlocked = db.get_player_titles(character_id).await.unwrap_or_default();
                if !unlocked.iter().any(|id| id == &title_id) {
                    self.send_system_message(player_id, "You haven't unlocked that title.")
                        .await;
                    return;
                }

                // Save to DB
                if let Err(e) = db.set_active_title(character_id, Some(&title_id)).await {
                    tracing::error!("Failed to set active title: {}", e);
                    self.send_system_message(player_id, "Failed to set title.")
                        .await;
                    return;
                }

                // Update in-memory
                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.active_title = Some(display.to_string());
                    }
                }

                self.send_system_message(player_id, &format!("Title set to: {}", display))
                    .await;
            }
            "clear" => {
                if let Err(e) = db.set_active_title(character_id, None).await {
                    tracing::error!("Failed to clear active title: {}", e);
                    self.send_system_message(player_id, "Failed to clear title.")
                        .await;
                    return;
                }

                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(player_id) {
                        player.active_title = None;
                    }
                }

                self.send_system_message(player_id, "Title cleared.").await;
            }
            _ => {
                self.send_system_message(player_id, "Usage: /title <list|set <title_id>|clear>")
                    .await;
            }
        }
    }
}
