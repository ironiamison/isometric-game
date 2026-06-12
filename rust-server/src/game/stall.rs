use super::*;

impl GameRoom {
    pub(super) async fn force_close_stall(&self, player_id: &str) {
        let stall_data = {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                player.stall.take()
            } else {
                None
            }
        };

        if let Some(stall) = stall_data {
            let registry = self.item_registry.clone();
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                for stall_slot in stall.slots.iter().flatten() {
                    let leftover = player.inventory.add_item(
                        &stall_slot.item_id,
                        stall_slot.quantity,
                        &registry,
                    );
                    if leftover > 0 {
                        player
                            .bank
                            .add_item(&stall_slot.item_id, leftover, &registry);
                    }
                }
            }
        }
    }

    fn stall_slots_to_data(stall: &PlayerStall) -> Vec<crate::protocol::StallSlotData> {
        stall
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| {
                slot.as_ref().map(|s| crate::protocol::StallSlotData {
                    slot: i as u8,
                    item_id: s.item_id.clone(),
                    quantity: s.quantity,
                    price: s.price,
                })
            })
            .collect()
    }

    pub async fn handle_stall_open(&self, player_id: &str, name: &str) {
        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) if p.active && !p.is_dead => p,
                _ => return,
            };
            if player.stall.as_ref().is_some_and(|s| s.active) {
                drop(players);
                self.send_system_message(player_id, "You already have a shop open.")
                    .await;
                return;
            }
        }

        {
            let pt = self.player_trades.read().await;
            if pt.contains_key(player_id) {
                self.send_system_message(player_id, "Cannot open shop while trading.")
                    .await;
                return;
            }
        }

        {
            let instances = self.player_instances.read().await;
            if instances.contains_key(player_id) {
                self.send_system_message(player_id, "Cannot open shop in an instance.")
                    .await;
                return;
            }
        }

        let stall_name = if name.trim().is_empty() {
            "Shop".to_string()
        } else {
            name.chars().take(32).collect::<String>()
        };

        let slots;
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id) {
                if let Some(ref mut stall) = player.stall {
                    stall.name = stall_name.clone();
                    stall.active = true;
                    slots = Self::stall_slots_to_data(stall);
                } else {
                    let mut stall = PlayerStall::new(stall_name.clone());
                    stall.active = true;
                    slots = Self::stall_slots_to_data(&stall);
                    player.stall = Some(stall);
                }
            } else {
                return;
            }
        }

        self.send_to_player(
            player_id,
            ServerMessage::StallOpened {
                name: stall_name,
                slots,
            },
        )
        .await;
    }

    pub async fn handle_stall_close(&self, player_id: &str) {
        {
            let players = self.players.read().await;
            let player = match players.get(player_id) {
                Some(p) => p,
                None => return,
            };
            let stall = match &player.stall {
                Some(s) => s,
                None => return,
            };

            let item_count = stall.slots.iter().filter(|s| s.is_some()).count();
            let empty_inv_slots = player
                .inventory
                .slots
                .iter()
                .filter(|s| s.is_none())
                .count();
            if item_count > empty_inv_slots {
                drop(players);
                self.send_system_message(
                    player_id,
                    "Not enough inventory space. Remove items first.",
                )
                .await;
                return;
            }
        }

        let registry = self.item_registry.clone();
        {
            let mut players = self.players.write().await;
            if let Some(player) = players.get_mut(player_id)
                && let Some(stall) = player.stall.take()
            {
                for stall_slot in stall.slots.iter().flatten() {
                    player
                        .inventory
                        .add_item(&stall_slot.item_id, stall_slot.quantity, &registry);
                }
            }
        }

        let inv_update = {
            let players = self.players.read().await;
            players
                .get(player_id)
                .map(|p| (p.inventory.to_update(), p.inventory.gold))
        };
        if let Some((slots, gold)) = inv_update {
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
        self.send_to_player(
            player_id,
            ServerMessage::StallClosed {
                reason: "Shop closed.".to_string(),
            },
        )
        .await;
    }

    pub async fn handle_stall_set_item(
        &self,
        player_id: &str,
        inventory_slot: u8,
        quantity: i32,
        price: i32,
    ) {
        if quantity <= 0 || price <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) => p,
            None => return,
        };

        if player.stall.is_none() {
            player.stall = Some(PlayerStall {
                name: String::new(),
                slots: std::iter::repeat_with(|| None)
                    .take(STALL_MAX_SLOTS)
                    .collect(),
                active: false,
            });
        }

        let (item_id, available) = match player.inventory.slots.get(inventory_slot as usize) {
            Some(Some(slot)) => (slot.item_id.clone(), slot.quantity),
            _ => {
                drop(players);
                self.send_system_message(player_id, "No item in that slot.")
                    .await;
                return;
            }
        };

        let qty = quantity.min(available);

        let stall = player.stall.as_mut().unwrap();
        let empty_slot = stall.slots.iter().position(|s| s.is_none());
        let slot_idx = match empty_slot {
            Some(i) => i,
            None => {
                drop(players);
                self.send_system_message(player_id, "Shop is full (max 10 slots).")
                    .await;
                return;
            }
        };

        if let Some(Some(inv_slot)) = player.inventory.slots.get_mut(inventory_slot as usize) {
            inv_slot.quantity -= qty;
            if inv_slot.quantity <= 0 {
                player.inventory.slots[inventory_slot as usize] = None;
            }
        }

        stall.slots[slot_idx] = Some(StallSlot {
            item_id,
            quantity: qty,
            price,
        });

        let stall_data = Self::stall_slots_to_data(stall);
        let inv_slots = player.inventory.to_update();
        let gold = player.inventory.gold;
        let pid = player_id.to_string();
        drop(players);

        self.send_to_player(&pid, ServerMessage::StallUpdate { slots: stall_data })
            .await;
        self.send_to_player(
            &pid,
            ServerMessage::InventoryUpdate {
                player_id: pid.clone(),
                slots: inv_slots,
                gold,
            },
        )
        .await;
    }

    pub async fn handle_stall_remove_item(&self, player_id: &str, stall_slot: u8) {
        let registry = self.item_registry.clone();

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.stall.is_some() => p,
            _ => return,
        };

        let stall = player.stall.as_mut().unwrap();
        let slot_data = match stall.slots.get(stall_slot as usize) {
            Some(Some(s)) => s.clone(),
            _ => return,
        };

        if !player
            .inventory
            .has_space_for(&slot_data.item_id, slot_data.quantity, &registry)
        {
            drop(players);
            self.send_system_message(player_id, "Inventory full.").await;
            return;
        }

        stall.slots[stall_slot as usize] = None;
        player
            .inventory
            .add_item(&slot_data.item_id, slot_data.quantity, &registry);

        let stall_data = Self::stall_slots_to_data(stall);
        let inv_slots = player.inventory.to_update();
        let gold = player.inventory.gold;
        let pid = player_id.to_string();
        drop(players);

        self.send_to_player(&pid, ServerMessage::StallUpdate { slots: stall_data })
            .await;
        self.send_to_player(
            &pid,
            ServerMessage::InventoryUpdate {
                player_id: pid.clone(),
                slots: inv_slots,
                gold,
            },
        )
        .await;
    }

    pub async fn handle_stall_update_price(&self, player_id: &str, stall_slot: u8, price: i32) {
        if price <= 0 {
            return;
        }

        let mut players = self.players.write().await;
        let player = match players.get_mut(player_id) {
            Some(p) if p.stall.is_some() => p,
            _ => return,
        };

        let stall = player.stall.as_mut().unwrap();
        if let Some(Some(slot)) = stall.slots.get_mut(stall_slot as usize) {
            slot.price = price;
        }

        let stall_data = Self::stall_slots_to_data(stall);
        drop(players);

        self.send_to_player(player_id, ServerMessage::StallUpdate { slots: stall_data })
            .await;
    }

    pub async fn handle_stall_browse(&self, player_id: &str, seller_id: &str) {
        if !self
            .players_share_interaction_context(player_id, seller_id, TRADE_MAX_DISTANCE)
            .await
        {
            self.send_system_message(player_id, "Too far away from that shop.")
                .await;
            return;
        }
        let players = self.players.read().await;
        let seller = match players.get(seller_id) {
            Some(p) if p.active && p.stall.as_ref().is_some_and(|s| s.active) => p,
            _ => {
                drop(players);
                self.send_system_message(player_id, "That player doesn't have a shop open.")
                    .await;
                return;
            }
        };

        let stall = seller.stall.as_ref().unwrap();
        let items = Self::stall_slots_to_data(stall);
        let seller_name = seller.name.clone();
        let stall_name = stall.name.clone();
        drop(players);

        self.send_to_player(
            player_id,
            ServerMessage::StallBrowseData {
                seller_id: seller_id.to_string(),
                seller_name,
                stall_name,
                items,
            },
        )
        .await;
    }

    pub async fn handle_stall_buy(
        &self,
        buyer_id: &str,
        seller_id: &str,
        stall_slot: u8,
        quantity: i32,
        expected_price: i32,
    ) {
        if quantity <= 0 || buyer_id == seller_id {
            return;
        }
        if !self
            .players_share_interaction_context(buyer_id, seller_id, TRADE_MAX_DISTANCE)
            .await
        {
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: String::new(),
                    quantity: 0,
                    total_price: 0,
                    error: Some("Too far away from that shop.".to_string()),
                },
            )
            .await;
            return;
        }

        let registry = self.item_registry.clone();
        let mut players = self.players.write().await;

        let (item_id, available_qty, price_per) = {
            let seller = match players.get(seller_id) {
                Some(p) if p.active && p.stall.as_ref().is_some_and(|s| s.active) => p,
                _ => {
                    drop(players);
                    self.send_to_player(
                        buyer_id,
                        ServerMessage::StallBuyResult {
                            success: false,
                            item_id: String::new(),
                            quantity: 0,
                            total_price: 0,
                            error: Some("Shop no longer available.".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            };
            let stall = seller.stall.as_ref().unwrap();
            match stall.slots.get(stall_slot as usize) {
                Some(Some(slot)) => (slot.item_id.clone(), slot.quantity, slot.price),
                _ => {
                    drop(players);
                    self.send_to_player(
                        buyer_id,
                        ServerMessage::StallBuyResult {
                            success: false,
                            item_id: String::new(),
                            quantity: 0,
                            total_price: 0,
                            error: Some("Item no longer available.".to_string()),
                        },
                    )
                    .await;
                    return;
                }
            }
        };

        let buy_qty = quantity.min(available_qty);
        if price_per != expected_price {
            drop(players);
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: item_id.clone(),
                    quantity: buy_qty,
                    total_price: 0,
                    error: Some("The listing price changed. Review it and try again.".to_string()),
                },
            )
            .await;
            return;
        }

        let Some(total_price) = item::checked_gold_total(price_per, buy_qty) else {
            drop(players);
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: item_id.clone(),
                    quantity: buy_qty,
                    total_price: 0,
                    error: Some("Invalid listing total.".to_string()),
                },
            )
            .await;
            return;
        };

        let buyer = match players.get(buyer_id) {
            Some(p) if p.active => p,
            _ => return,
        };

        let Some(new_buyer_gold) = item::checked_gold_debit(buyer.inventory.gold, total_price)
        else {
            drop(players);
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: item_id.clone(),
                    quantity: buy_qty,
                    total_price,
                    error: Some("Not enough gold.".to_string()),
                },
            )
            .await;
            return;
        };

        let seller_gold = players
            .get(seller_id)
            .map(|seller| seller.inventory.gold)
            .unwrap_or(-1);
        let Some(new_seller_gold) = item::checked_gold_credit(seller_gold, total_price) else {
            drop(players);
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: item_id.clone(),
                    quantity: buy_qty,
                    total_price,
                    error: Some("Seller cannot receive that much gold.".to_string()),
                },
            )
            .await;
            return;
        };

        if !buyer.inventory.has_space_for(&item_id, buy_qty, &registry) {
            drop(players);
            self.send_to_player(
                buyer_id,
                ServerMessage::StallBuyResult {
                    success: false,
                    item_id: item_id.clone(),
                    quantity: buy_qty,
                    total_price,
                    error: Some("Inventory full.".to_string()),
                },
            )
            .await;
            return;
        }

        if let Some(seller) = players.get_mut(seller_id) {
            if let Some(stall) = seller.stall.as_mut()
                && let Some(Some(slot)) = stall.slots.get_mut(stall_slot as usize)
            {
                slot.quantity -= buy_qty;
                if slot.quantity <= 0 {
                    stall.slots[stall_slot as usize] = None;
                }
            }
            seller.inventory.gold = new_seller_gold;
        }

        if let Some(buyer) = players.get_mut(buyer_id) {
            buyer.inventory.gold = new_buyer_gold;
            buyer.inventory.add_item(&item_id, buy_qty, &registry);
        }

        let buyer_inv = players
            .get(buyer_id)
            .map(|p| (p.inventory.to_update(), p.inventory.gold));
        let seller_inv = players
            .get(seller_id)
            .map(|p| (p.inventory.to_update(), p.inventory.gold));
        let stall_update = players
            .get(seller_id)
            .and_then(|p| p.stall.as_ref())
            .map(Self::stall_slots_to_data);
        let new_qty = players
            .get(seller_id)
            .and_then(|p| p.stall.as_ref())
            .and_then(|s| s.slots.get(stall_slot as usize))
            .map(|s| s.as_ref().map_or(0, |slot| slot.quantity))
            .unwrap_or(0);
        let buyer_name = players
            .get(buyer_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        drop(players);

        self.send_to_player(
            buyer_id,
            ServerMessage::StallBuyResult {
                success: true,
                item_id: item_id.clone(),
                quantity: buy_qty,
                total_price,
                error: None,
            },
        )
        .await;

        if let Some((slots, gold)) = buyer_inv {
            self.send_to_player(
                buyer_id,
                ServerMessage::InventoryUpdate {
                    player_id: buyer_id.to_string(),
                    slots,
                    gold,
                },
            )
            .await;
        }
        if let Some((slots, gold)) = seller_inv {
            self.send_to_player(
                seller_id,
                ServerMessage::InventoryUpdate {
                    player_id: seller_id.to_string(),
                    slots,
                    gold,
                },
            )
            .await;
        }

        if let Some(slots) = stall_update {
            self.send_to_player(seller_id, ServerMessage::StallUpdate { slots })
                .await;
        }

        self.send_to_player(
            seller_id,
            ServerMessage::StallSaleNotification {
                item_id: item_id.clone(),
                quantity: buy_qty,
                gold_received: total_price,
                buyer_name,
            },
        )
        .await;

        self.send_to_player(
            buyer_id,
            ServerMessage::StallItemUpdate {
                seller_id: seller_id.to_string(),
                stall_slot,
                new_quantity: new_qty,
            },
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_slots_to_data_skips_empty_slots_and_preserves_indices() {
        let mut stall = PlayerStall::new("Smithy's".to_string());
        stall.slots[0] = Some(StallSlot {
            item_id: "bread".to_string(),
            quantity: 3,
            price: 5,
        });
        stall.slots[4] = Some(StallSlot {
            item_id: "bronze_sword".to_string(),
            quantity: 1,
            price: 20,
        });

        let data = GameRoom::stall_slots_to_data(&stall);

        assert_eq!(data.len(), 2);
        assert_eq!(data[0].slot, 0);
        assert_eq!(data[0].item_id, "bread");
        assert_eq!(data[0].quantity, 3);
        assert_eq!(data[0].price, 5);
        assert_eq!(data[1].slot, 4);
        assert_eq!(data[1].item_id, "bronze_sword");
        assert_eq!(data[1].quantity, 1);
        assert_eq!(data[1].price, 20);
    }
}
