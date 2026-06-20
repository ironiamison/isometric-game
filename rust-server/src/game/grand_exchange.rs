use super::*;
use crate::protocol::{GeMarketData, GeOfferData};

/// SOLST decimal places (mirrors solstead_chain::TOKEN_DECIMALS).
const GE_DECIMALS: u8 = 6;
/// Guardrails on order parameters (base units / unit counts).
const GE_MAX_PRICE: i64 = 1_000_000_000_000; // 1,000,000 SOLST at 6 decimals
const GE_MAX_QUANTITY: i64 = 1_000_000;
/// Max market rows sent to the client per refresh.
const GE_MARKET_LIMIT: i64 = 120;

impl GameRoom {
    /// Resolve the account id for a connected player.
    async fn ge_account_id(&self, player_id: &str) -> Option<i64> {
        let players = self.players.read().await;
        players.get(player_id).map(|p| p.account_id)
    }

    /// Persist a character immediately (Grand Exchange moves real value).
    async fn ge_persist(&self, player_id: &str) {
        if let (Some(db), Some(character_id)) =
            (self.db.as_ref(), Self::parse_character_id(player_id))
            && let Some(save) = self.get_player_save_data(player_id).await
            && let Err(e) = db.save_character(character_id, &save, 0).await
        {
            tracing::error!("Failed to persist GE result for {}: {}", player_id, e);
        }
    }

    /// Build and unicast the player's Grand Exchange snapshot.
    pub(super) async fn send_grand_exchange_data(&self, player_id: &str) {
        let Some(db) = self.db.as_ref() else {
            self.send_system_message(player_id, "The Grand Exchange is unavailable.")
                .await;
            return;
        };
        let Some(account_id) = self.ge_account_id(player_id).await else {
            return;
        };

        let balance = db.get_chain_balance(account_id).await.unwrap_or(0);
        let offers = db
            .ge_offers_for_account(account_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|o| GeOfferData {
                id: o.id,
                side: o.side,
                item_id: o.item_id,
                price: o.price,
                quantity: o.quantity,
                remaining: o.remaining,
                collect_items: o.collect_items,
                status: o.status,
            })
            .collect();
        let market = db
            .ge_active_market(GE_MARKET_LIMIT)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|o| GeMarketData {
                side: o.side,
                item_id: o.item_id,
                price: o.price,
                quantity: o.remaining,
            })
            .collect();

        self.send_to_player(
            player_id,
            ServerMessage::GrandExchangeData {
                balance,
                decimals: GE_DECIMALS,
                offers,
                market,
            },
        )
        .await;
    }

    pub async fn handle_ge_open(&self, player_id: &str) {
        self.send_grand_exchange_data(player_id).await;
    }

    pub async fn handle_ge_place_offer(
        &self,
        player_id: &str,
        side: &str,
        item_id: &str,
        price: i64,
        quantity: i64,
    ) {
        let Some(db) = self.db.as_ref() else {
            return;
        };

        // Validate parameters.
        if price <= 0 || price > GE_MAX_PRICE {
            self.send_ge_error(player_id, "Invalid price.").await;
            return;
        }
        if quantity <= 0 || quantity > GE_MAX_QUANTITY {
            self.send_ge_error(player_id, "Invalid quantity.").await;
            return;
        }
        if self.item_registry.get(item_id).is_none() {
            self.send_ge_error(player_id, "Unknown item.").await;
            return;
        }

        let Some(account_id) = self.ge_account_id(player_id).await else {
            return;
        };
        let Some(character_id) = Self::parse_character_id(player_id) else {
            self.send_ge_error(player_id, "Guests cannot use the Grand Exchange.")
                .await;
            return;
        };

        match side {
            "buy" => {
                match db
                    .ge_place_buy(account_id, character_id, item_id, price, quantity)
                    .await
                {
                    Ok(Ok(result)) => {
                        let msg = if result.filled > 0 {
                            format!("Buy offer placed — {} bought instantly.", result.filled)
                        } else {
                            "Buy offer placed.".to_string()
                        };
                        self.send_ge_ok(player_id, &msg).await;
                    }
                    Ok(Err(reason)) => {
                        self.send_ge_error(player_id, &reason).await;
                    }
                    Err(e) => {
                        tracing::error!("GE buy failed for {}: {}", player_id, e);
                        self.send_ge_error(player_id, "Grand Exchange error.").await;
                    }
                }
            }
            "sell" => {
                // Escrow the items off the seller's inventory first.
                let registry = self.item_registry.clone();
                let escrowed = {
                    let mut players = self.players.write().await;
                    match players.get_mut(player_id) {
                        Some(player) => {
                            if !player.inventory.has_item(item_id, quantity as i32) {
                                false
                            } else {
                                player.inventory.remove_item(item_id, quantity as i32)
                            }
                        }
                        None => false,
                    }
                };
                if !escrowed {
                    self.send_ge_error(player_id, "You don't have that many to sell.")
                        .await;
                    return;
                }
                self.ge_persist(player_id).await;
                self.send_inventory_update(player_id).await;

                match db
                    .ge_place_sell(account_id, character_id, item_id, price, quantity)
                    .await
                {
                    Ok(result) => {
                        let msg = if result.filled > 0 {
                            format!("Sell offer placed — {} sold instantly.", result.filled)
                        } else {
                            "Sell offer placed.".to_string()
                        };
                        self.send_ge_ok(player_id, &msg).await;
                    }
                    Err(e) => {
                        tracing::error!("GE sell failed for {}: {}", player_id, e);
                        // Refund escrowed items back to the seller (space was just freed
                        // by the escrow removal, so this fully fits).
                        {
                            let mut players = self.players.write().await;
                            if let Some(player) = players.get_mut(player_id) {
                                player.inventory.add_item(item_id, quantity as i32, &registry);
                            }
                        }
                        self.ge_persist(player_id).await;
                        self.send_inventory_update(player_id).await;
                        self.send_ge_error(player_id, "Grand Exchange error.").await;
                    }
                }
            }
            _ => {
                self.send_ge_error(player_id, "Invalid offer type.").await;
            }
        }

        self.send_grand_exchange_data(player_id).await;
    }

    pub async fn handle_ge_cancel_offer(&self, player_id: &str, offer_id: i64) {
        let Some(db) = self.db.as_ref() else {
            return;
        };
        let Some(account_id) = self.ge_account_id(player_id).await else {
            return;
        };

        match db.ge_cancel(account_id, offer_id).await {
            Ok(Ok(result)) => {
                let msg = if result.side == "buy" {
                    "Offer cancelled — SOLST refunded.".to_string()
                } else if result.returned_to_collect > 0 {
                    format!(
                        "Offer cancelled — {} item(s) waiting to collect.",
                        result.returned_to_collect
                    )
                } else {
                    "Offer cancelled.".to_string()
                };
                self.send_ge_ok(player_id, &msg).await;
            }
            Ok(Err(reason)) => {
                self.send_ge_error(player_id, &reason).await;
            }
            Err(e) => {
                tracing::error!("GE cancel failed for {}: {}", player_id, e);
                self.send_ge_error(player_id, "Grand Exchange error.").await;
            }
        }

        self.send_grand_exchange_data(player_id).await;
    }

    pub async fn handle_ge_collect(&self, player_id: &str, offer_id: i64) {
        let Some(db) = self.db.as_ref() else {
            return;
        };
        let Some(account_id) = self.ge_account_id(player_id).await else {
            return;
        };

        let offer = match db.ge_get_offer(offer_id).await {
            Ok(Some(o)) => o,
            _ => {
                self.send_ge_error(player_id, "Offer not found.").await;
                return;
            }
        };
        if offer.account_id != account_id {
            self.send_ge_error(player_id, "That is not your offer.").await;
            return;
        }
        if offer.collect_items <= 0 {
            self.send_ge_error(player_id, "Nothing to collect.").await;
            return;
        }

        let registry = self.item_registry.clone();
        let taken = {
            let mut players = self.players.write().await;
            match players.get_mut(player_id) {
                Some(player) => {
                    let space = player.inventory.available_space_for(&offer.item_id, &registry);
                    let take = offer.collect_items.min(space as i64);
                    if take > 0 {
                        player.inventory.add_item(&offer.item_id, take as i32, &registry);
                    }
                    take
                }
                None => 0,
            }
        };

        if taken <= 0 {
            self.send_ge_error(player_id, "Not enough inventory space.")
                .await;
            return;
        }

        if let Err(e) = db.ge_reduce_collect(offer_id, taken).await {
            tracing::error!("GE collect reduce failed for {}: {}", player_id, e);
        }
        self.ge_persist(player_id).await;
        self.send_inventory_update(player_id).await;
        self.send_ge_ok(player_id, &format!("Collected {} item(s).", taken))
            .await;
        self.send_grand_exchange_data(player_id).await;
    }

    /// Send the player's current inventory state (unicast).
    async fn send_inventory_update(&self, player_id: &str) {
        let update = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|p| (p.inventory.to_update(), p.inventory.gold))
        };
        if let Some((slots, gold)) = update {
            self.send_to_player(
                player_id,
                ServerMessage::InventoryUpdate {
                    player_id: player_id.to_string(),
                    slots,
                    gold,
                },
            )
            .await;
        }
    }

    async fn send_ge_ok(&self, player_id: &str, message: &str) {
        self.send_to_player(
            player_id,
            ServerMessage::GeResult {
                success: true,
                message: message.to_string(),
            },
        )
        .await;
    }

    async fn send_ge_error(&self, player_id: &str, message: &str) {
        self.send_to_player(
            player_id,
            ServerMessage::GeResult {
                success: false,
                message: message.to_string(),
            },
        )
        .await;
    }
}
