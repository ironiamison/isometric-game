use super::*;

impl GameRoom {
    /// Check if a player's inventory slot is locked in an active trade offer.
    pub(super) async fn is_slot_in_trade(&self, player_id: &str, slot_index: u8) -> bool {
        let player_trades = self.player_trades.read().await;
        if let Some(trade_id) = player_trades.get(player_id) {
            let trades = self.trades.read().await;
            if let Some(session) = trades.get(trade_id) {
                let offer = if session.player_a == player_id {
                    &session.offer_a
                } else {
                    &session.offer_b
                };
                return offer.items.iter().any(|item| item.inv_slot == slot_index);
            }
        }
        false
    }

    fn make_trade_offer_items(offer: &TradeOffer) -> Vec<crate::protocol::TradeOfferItemData> {
        offer
            .items
            .iter()
            .map(|e| crate::protocol::TradeOfferItemData {
                slot_index: e.inv_slot,
                item_id: e.item_id.clone(),
                quantity: e.quantity,
            })
            .collect()
    }

    pub(super) async fn cancel_trade_for_player(&self, player_id: &str, reason: &str) {
        let trade_id = {
            let pt = self.player_trades.read().await;
            pt.get(player_id).cloned()
        };
        if let Some(trade_id) = trade_id {
            let session = {
                let mut trades = self.trades.write().await;
                trades.remove(&trade_id)
            };
            if let Some(session) = session {
                {
                    let mut pt = self.player_trades.write().await;
                    pt.remove(&session.player_a);
                    pt.remove(&session.player_b);
                }
                let msg = ServerMessage::TradeCancelled {
                    reason: reason.to_string(),
                };
                self.send_to_player(&session.player_a, msg.clone()).await;
                self.send_to_player(&session.player_b, msg).await;
            }
        }

        let mut requests = self.trade_requests.write().await;
        requests.retain(|_, (requester, _)| requester != player_id);
    }

    pub async fn handle_trade_request(&self, player_id: &str, target_id: &str) {
        let (requester_name, valid) = {
            let players = self.players.read().await;
            let requester = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };
            let target = match players.get(target_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => {
                    return;
                }
            };
            let dx = (requester.x - target.x).abs();
            let dy = (requester.y - target.y).abs();
            if dx > TRADE_MAX_DISTANCE || dy > TRADE_MAX_DISTANCE {
                return;
            }
            let name = requester.name.clone();
            if target.stall.as_ref().is_some_and(|s| s.active) {
                drop(players);
                self.send_system_message(player_id, "That player is running a shop.")
                    .await;
                return;
            }
            if requester.stall.as_ref().is_some_and(|s| s.active) {
                drop(players);
                self.send_system_message(player_id, "Close your shop before trading.")
                    .await;
                return;
            }
            (name, true)
        };
        if !valid {
            return;
        }

        {
            let pt = self.player_trades.read().await;
            if pt.contains_key(player_id) {
                self.send_system_message(player_id, "You are already in a trade.")
                    .await;
                return;
            }
            if pt.contains_key(target_id) {
                self.send_system_message(player_id, "That player is already trading.")
                    .await;
                return;
            }
        }

        let current_tick = *self.tick.read().await;
        {
            let mut requests = self.trade_requests.write().await;
            requests.insert(target_id.to_string(), (player_id.to_string(), current_tick));
        }

        self.send_to_player(
            target_id,
            ServerMessage::TradeRequestReceived {
                requester_id: player_id.to_string(),
                requester_name,
            },
        )
        .await;
    }

    pub async fn handle_trade_accept_request(&self, player_id: &str, requester_id: &str) {
        let valid_request = {
            let requests = self.trade_requests.read().await;
            requests
                .get(player_id)
                .map(|(rid, _)| rid == requester_id)
                .unwrap_or(false)
        };
        if !valid_request {
            self.send_system_message(player_id, "No pending trade request from that player.")
                .await;
            return;
        }

        {
            let mut requests = self.trade_requests.write().await;
            requests.remove(player_id);
        }

        {
            let pt = self.player_trades.read().await;
            if pt.contains_key(player_id) || pt.contains_key(requester_id) {
                self.send_system_message(player_id, "Trade no longer available.")
                    .await;
                return;
            }
        }

        let (name_a, name_b) = {
            let players = self.players.read().await;
            let pa = match players.get(requester_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };
            let pb = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };
            let dx = (pa.x - pb.x).abs();
            let dy = (pa.y - pb.y).abs();
            if dx > TRADE_MAX_DISTANCE || dy > TRADE_MAX_DISTANCE {
                self.send_system_message(player_id, "Too far away to trade.")
                    .await;
                return;
            }
            (pa.name.clone(), pb.name.clone())
        };

        let trade_id = format!("trade_{}_{}", requester_id, player_id);
        let session = TradeSession {
            player_a: requester_id.to_string(),
            player_b: player_id.to_string(),
            offer_a: TradeOffer::new(),
            offer_b: TradeOffer::new(),
        };

        {
            let mut trades = self.trades.write().await;
            trades.insert(trade_id.clone(), session);
        }
        {
            let mut pt = self.player_trades.write().await;
            pt.insert(requester_id.to_string(), trade_id.clone());
            pt.insert(player_id.to_string(), trade_id.clone());
        }

        self.send_to_player(
            requester_id,
            ServerMessage::TradeOpened {
                trade_id: trade_id.clone(),
                partner_id: player_id.to_string(),
                partner_name: name_b,
            },
        )
        .await;
        self.send_to_player(
            player_id,
            ServerMessage::TradeOpened {
                trade_id,
                partner_id: requester_id.to_string(),
                partner_name: name_a,
            },
        )
        .await;
    }

    pub async fn handle_trade_decline_request(&self, player_id: &str, requester_id: &str) {
        let removed = {
            let mut requests = self.trade_requests.write().await;
            if requests
                .get(player_id)
                .map(|(rid, _)| rid == requester_id)
                .unwrap_or(false)
            {
                requests.remove(player_id);
                true
            } else {
                false
            }
        };
        if removed {
            self.send_system_message(requester_id, "Trade request declined.")
                .await;
        }
    }

    pub async fn handle_trade_offer_item(&self, player_id: &str, slot_index: u8, quantity: i32) {
        if quantity <= 0 {
            return;
        }

        let trade_id = {
            let pt = self.player_trades.read().await;
            match pt.get(player_id) {
                Some(id) => id.clone(),
                None => return,
            }
        };

        let item_info = {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };
            match player.inventory.slots.get(slot_index as usize) {
                Some(Some(slot)) => Some((slot.item_id.clone(), slot.quantity)),
                _ => None,
            }
        };

        let (item_id, available_qty) = match item_info {
            Some(info) => info,
            None => {
                self.send_system_message(player_id, "No item in that slot.")
                    .await;
                return;
            }
        };

        let qty = quantity.min(available_qty);

        let mut trades = self.trades.write().await;
        if let Some(session) = trades.get_mut(&trade_id) {
            let (my_offer, partner_id) = if session.player_a == player_id {
                (&mut session.offer_a, session.player_b.clone())
            } else {
                (&mut session.offer_b, session.player_a.clone())
            };

            if my_offer.items.iter().any(|e| e.inv_slot == slot_index) {
                drop(trades);
                self.send_system_message(player_id, "That slot is already in the offer.")
                    .await;
                return;
            }

            my_offer.items.push(TradeOfferEntry {
                inv_slot: slot_index,
                item_id,
                quantity: qty,
            });

            session.offer_a.accepted = false;
            session.offer_b.accepted = false;

            let my_offer = if session.player_a == player_id {
                &session.offer_a
            } else {
                &session.offer_b
            };
            let my_items = Self::make_trade_offer_items(my_offer);
            let my_gold = my_offer.gold;
            let partner_view_items = Self::make_trade_offer_items(my_offer);
            let partner_view_gold = my_offer.gold;

            drop(trades);

            self.send_to_player(
                player_id,
                ServerMessage::TradeMyOfferUpdate {
                    my_items,
                    my_gold,
                    my_accepted: false,
                },
            )
            .await;
            self.send_to_player(
                &partner_id,
                ServerMessage::TradeOfferUpdate {
                    partner_items: partner_view_items,
                    partner_gold: partner_view_gold,
                    partner_accepted: false,
                },
            )
            .await;
        }
    }

    pub async fn handle_trade_remove_item(&self, player_id: &str, offer_index: u8) {
        let trade_id = {
            let pt = self.player_trades.read().await;
            match pt.get(player_id) {
                Some(id) => id.clone(),
                None => return,
            }
        };

        let mut trades = self.trades.write().await;
        if let Some(session) = trades.get_mut(&trade_id) {
            let (my_offer, partner_id) = if session.player_a == player_id {
                (&mut session.offer_a, session.player_b.clone())
            } else {
                (&mut session.offer_b, session.player_a.clone())
            };

            if (offer_index as usize) < my_offer.items.len() {
                my_offer.items.remove(offer_index as usize);
            } else {
                return;
            }

            session.offer_a.accepted = false;
            session.offer_b.accepted = false;

            let my_offer_ref = if session.player_a == player_id {
                &session.offer_a
            } else {
                &session.offer_b
            };
            let my_items = Self::make_trade_offer_items(my_offer_ref);
            let my_gold = my_offer_ref.gold;
            let partner_view_items = Self::make_trade_offer_items(my_offer_ref);
            let partner_view_gold = my_offer_ref.gold;

            drop(trades);

            self.send_to_player(
                player_id,
                ServerMessage::TradeMyOfferUpdate {
                    my_items,
                    my_gold,
                    my_accepted: false,
                },
            )
            .await;
            self.send_to_player(
                &partner_id,
                ServerMessage::TradeOfferUpdate {
                    partner_items: partner_view_items,
                    partner_gold: partner_view_gold,
                    partner_accepted: false,
                },
            )
            .await;
        }
    }

    pub async fn handle_trade_offer_gold(&self, player_id: &str, amount: i32) {
        if amount < 0 {
            return;
        }

        let trade_id = {
            let pt = self.player_trades.read().await;
            match pt.get(player_id) {
                Some(id) => id.clone(),
                None => return,
            }
        };

        let max_gold = {
            let players = self.players.read().await;
            match players.get(player_id) {
                Some(p) => p.inventory.gold,
                None => return,
            }
        };
        let gold = amount.min(max_gold.max(0));

        let mut trades = self.trades.write().await;
        if let Some(session) = trades.get_mut(&trade_id) {
            let (my_offer, partner_id) = if session.player_a == player_id {
                (&mut session.offer_a, session.player_b.clone())
            } else {
                (&mut session.offer_b, session.player_a.clone())
            };

            my_offer.gold = gold;
            session.offer_a.accepted = false;
            session.offer_b.accepted = false;

            let my_offer_ref = if session.player_a == player_id {
                &session.offer_a
            } else {
                &session.offer_b
            };
            let my_items = Self::make_trade_offer_items(my_offer_ref);
            let my_gold = my_offer_ref.gold;
            let partner_view_items = Self::make_trade_offer_items(my_offer_ref);
            let partner_view_gold = my_offer_ref.gold;

            drop(trades);

            self.send_to_player(
                player_id,
                ServerMessage::TradeMyOfferUpdate {
                    my_items,
                    my_gold,
                    my_accepted: false,
                },
            )
            .await;
            self.send_to_player(
                &partner_id,
                ServerMessage::TradeOfferUpdate {
                    partner_items: partner_view_items,
                    partner_gold: partner_view_gold,
                    partner_accepted: false,
                },
            )
            .await;
        }
    }

    pub async fn handle_trade_accept(&self, player_id: &str) {
        let trade_id = {
            let pt = self.player_trades.read().await;
            match pt.get(player_id) {
                Some(id) => id.clone(),
                None => return,
            }
        };

        let should_complete;
        let partner_id;
        {
            let mut trades = self.trades.write().await;
            if let Some(session) = trades.get_mut(&trade_id) {
                let (my_offer, other_offer, pid) = if session.player_a == player_id {
                    (
                        &mut session.offer_a,
                        &session.offer_b,
                        session.player_b.clone(),
                    )
                } else {
                    (
                        &mut session.offer_b,
                        &session.offer_a,
                        session.player_a.clone(),
                    )
                };
                partner_id = pid;

                my_offer.accepted = true;
                should_complete = other_offer.accepted;

                if !should_complete {
                    let partner_items = Self::make_trade_offer_items(my_offer);
                    let partner_gold = my_offer.gold;
                    drop(trades);

                    self.send_to_player(
                        &partner_id,
                        ServerMessage::TradeOfferUpdate {
                            partner_items,
                            partner_gold,
                            partner_accepted: true,
                        },
                    )
                    .await;

                    let trade_id2 = {
                        let pt = self.player_trades.read().await;
                        pt.get(player_id).cloned()
                    };
                    if let Some(tid) = trade_id2 {
                        let trades = self.trades.read().await;
                        if let Some(session) = trades.get(&tid) {
                            let my_items =
                                Self::make_trade_offer_items(if session.player_a == player_id {
                                    &session.offer_a
                                } else {
                                    &session.offer_b
                                });
                            let my_gold = if session.player_a == player_id {
                                session.offer_a.gold
                            } else {
                                session.offer_b.gold
                            };
                            drop(trades);
                            self.send_to_player(
                                player_id,
                                ServerMessage::TradeMyOfferUpdate {
                                    my_items,
                                    my_gold,
                                    my_accepted: true,
                                },
                            )
                            .await;
                        }
                    }
                    return;
                }
            } else {
                return;
            }
        }

        self.execute_trade(&trade_id).await;
    }

    async fn execute_trade(&self, trade_id: &str) {
        let session = {
            let mut trades = self.trades.write().await;
            match trades.remove(trade_id) {
                Some(s) => s,
                None => return,
            }
        };

        {
            let mut pt = self.player_trades.write().await;
            pt.remove(&session.player_a);
            pt.remove(&session.player_b);
        }

        let registry = self.item_registry.clone();

        let mut players = self.players.write().await;
        let (pa, pb) = match (
            players.get(&session.player_a),
            players.get(&session.player_b),
        ) {
            (Some(a), Some(b)) => {
                for item in &session.offer_a.items {
                    match a.inventory.slots.get(item.inv_slot as usize) {
                        Some(Some(slot))
                            if slot.item_id == item.item_id && slot.quantity >= item.quantity => {}
                        _ => {
                            drop(players);
                            let msg = ServerMessage::TradeCancelled {
                                reason: "Items changed during trade.".to_string(),
                            };
                            self.send_to_player(&session.player_a, msg.clone()).await;
                            self.send_to_player(&session.player_b, msg).await;
                            return;
                        }
                    }
                }
                for item in &session.offer_b.items {
                    match b.inventory.slots.get(item.inv_slot as usize) {
                        Some(Some(slot))
                            if slot.item_id == item.item_id && slot.quantity >= item.quantity => {}
                        _ => {
                            drop(players);
                            let msg = ServerMessage::TradeCancelled {
                                reason: "Items changed during trade.".to_string(),
                            };
                            self.send_to_player(&session.player_a, msg.clone()).await;
                            self.send_to_player(&session.player_b, msg).await;
                            return;
                        }
                    }
                }
                if a.inventory.gold < session.offer_a.gold
                    || b.inventory.gold < session.offer_b.gold
                {
                    drop(players);
                    let msg = ServerMessage::TradeCancelled {
                        reason: "Insufficient gold.".to_string(),
                    };
                    self.send_to_player(&session.player_a, msg.clone()).await;
                    self.send_to_player(&session.player_b, msg).await;
                    return;
                }
                if item::checked_gold_debit(a.inventory.gold, session.offer_a.gold)
                    .and_then(|gold| item::checked_gold_credit(gold, session.offer_b.gold))
                    .is_none()
                    || item::checked_gold_debit(b.inventory.gold, session.offer_b.gold)
                        .and_then(|gold| item::checked_gold_credit(gold, session.offer_a.gold))
                        .is_none()
                {
                    drop(players);
                    let msg = ServerMessage::TradeCancelled {
                        reason: "Trade would exceed the gold limit.".to_string(),
                    };
                    self.send_to_player(&session.player_a, msg.clone()).await;
                    self.send_to_player(&session.player_b, msg).await;
                    return;
                }
                (session.player_a.clone(), session.player_b.clone())
            }
            _ => return,
        };

        for item in session.offer_a.items.iter().rev() {
            if let Some(player) = players.get_mut(&pa) {
                if let Some(Some(slot)) = player.inventory.slots.get_mut(item.inv_slot as usize) {
                    slot.quantity -= item.quantity;
                    if slot.quantity <= 0 {
                        player.inventory.slots[item.inv_slot as usize] = None;
                    }
                }
            }
        }
        for item in session.offer_b.items.iter().rev() {
            if let Some(player) = players.get_mut(&pb) {
                if let Some(Some(slot)) = player.inventory.slots.get_mut(item.inv_slot as usize) {
                    slot.quantity -= item.quantity;
                    if slot.quantity <= 0 {
                        player.inventory.slots[item.inv_slot as usize] = None;
                    }
                }
            }
        }

        if let Some(player_a) = players.get_mut(&pa) {
            player_a.inventory.gold =
                item::checked_gold_debit(player_a.inventory.gold, session.offer_a.gold)
                    .and_then(|gold| item::checked_gold_credit(gold, session.offer_b.gold))
                    .expect("trade balances were validated before mutation");
        }
        if let Some(player_b) = players.get_mut(&pb) {
            player_b.inventory.gold =
                item::checked_gold_debit(player_b.inventory.gold, session.offer_b.gold)
                    .and_then(|gold| item::checked_gold_credit(gold, session.offer_a.gold))
                    .expect("trade balances were validated before mutation");
        }

        for item in &session.offer_b.items {
            if let Some(player) = players.get_mut(&pa) {
                player
                    .inventory
                    .add_item(&item.item_id, item.quantity, &registry);
            }
        }
        for item in &session.offer_a.items {
            if let Some(player) = players.get_mut(&pb) {
                player
                    .inventory
                    .add_item(&item.item_id, item.quantity, &registry);
            }
        }

        let inv_a = players
            .get(&pa)
            .map(|p| (p.inventory.to_update(), p.inventory.gold));
        let inv_b = players
            .get(&pb)
            .map(|p| (p.inventory.to_update(), p.inventory.gold));
        drop(players);

        let items_a_received: Vec<crate::protocol::TradeOfferItemData> = session
            .offer_b
            .items
            .iter()
            .map(|e| crate::protocol::TradeOfferItemData {
                slot_index: e.inv_slot,
                item_id: e.item_id.clone(),
                quantity: e.quantity,
            })
            .collect();
        let items_b_received: Vec<crate::protocol::TradeOfferItemData> = session
            .offer_a
            .items
            .iter()
            .map(|e| crate::protocol::TradeOfferItemData {
                slot_index: e.inv_slot,
                item_id: e.item_id.clone(),
                quantity: e.quantity,
            })
            .collect();

        self.send_to_player(
            &pa,
            ServerMessage::TradeCompleted {
                items_received: items_a_received,
                gold_received: session.offer_b.gold,
            },
        )
        .await;
        self.send_to_player(
            &pb,
            ServerMessage::TradeCompleted {
                items_received: items_b_received,
                gold_received: session.offer_a.gold,
            },
        )
        .await;

        if let Some((slots, gold)) = inv_a {
            self.send_to_player(
                &pa,
                ServerMessage::InventoryUpdate {
                    player_id: pa.clone(),
                    slots,
                    gold,
                },
            )
            .await;
        }
        if let Some((slots, gold)) = inv_b {
            self.send_to_player(
                &pb,
                ServerMessage::InventoryUpdate {
                    player_id: pb.clone(),
                    slots,
                    gold,
                },
            )
            .await;
        }
    }

    pub async fn handle_trade_cancel(&self, player_id: &str) {
        self.cancel_trade_for_player(player_id, "Trade cancelled.")
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_trade_offer_items_preserves_slot_item_and_quantity() {
        let offer = TradeOffer {
            items: vec![
                TradeOfferEntry {
                    inv_slot: 3,
                    item_id: "bronze_sword".to_string(),
                    quantity: 2,
                },
                TradeOfferEntry {
                    inv_slot: 7,
                    item_id: "bread".to_string(),
                    quantity: 5,
                },
            ],
            gold: 42,
            accepted: false,
        };

        let data = GameRoom::make_trade_offer_items(&offer);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].slot_index, 3);
        assert_eq!(data[0].item_id, "bronze_sword");
        assert_eq!(data[0].quantity, 2);
        assert_eq!(data[1].slot_index, 7);
        assert_eq!(data[1].item_id, "bread");
        assert_eq!(data[1].quantity, 5);
    }
}
