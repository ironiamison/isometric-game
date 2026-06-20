use super::*;

pub(super) fn encode(msg: &ServerMessage) -> Option<Value> {
    let value = match msg {
        ServerMessage::RecipeDefinitions { recipes } => {
            let mut map = Vec::new();
            let recipe_values: Vec<Value> = recipes
                .iter()
                .map(|r| {
                    let mut rmap = Vec::new();
                    rmap.push((
                        Value::String("id".into()),
                        Value::String(r.id.clone().into()),
                    ));
                    rmap.push((
                        Value::String("display_name".into()),
                        Value::String(r.display_name.clone().into()),
                    ));
                    rmap.push((
                        Value::String("description".into()),
                        Value::String(r.description.clone().into()),
                    ));
                    rmap.push((
                        Value::String("category".into()),
                        Value::String(r.category.clone().into()),
                    ));
                    if let Some(ref s) = r.section {
                        rmap.push((
                            Value::String("section".into()),
                            Value::String(s.clone().into()),
                        ));
                    }
                    rmap.push((
                        Value::String("level_required".into()),
                        Value::Integer((r.level_required as i64).into()),
                    ));

                    let ingredient_values: Vec<Value> = r
                        .ingredients
                        .iter()
                        .map(|i| {
                            let mut imap = Vec::new();
                            imap.push((
                                Value::String("item_id".into()),
                                Value::String(i.item_id.clone().into()),
                            ));
                            imap.push((
                                Value::String("item_name".into()),
                                Value::String(i.item_name.clone().into()),
                            ));
                            imap.push((
                                Value::String("count".into()),
                                Value::Integer((i.count as i64).into()),
                            ));
                            Value::Map(imap)
                        })
                        .collect();
                    rmap.push((
                        Value::String("ingredients".into()),
                        Value::Array(ingredient_values),
                    ));

                    let result_values: Vec<Value> = r
                        .results
                        .iter()
                        .map(|res| {
                            let mut resmap = Vec::new();
                            resmap.push((
                                Value::String("item_id".into()),
                                Value::String(res.item_id.clone().into()),
                            ));
                            resmap.push((
                                Value::String("item_name".into()),
                                Value::String(res.item_name.clone().into()),
                            ));
                            resmap.push((
                                Value::String("count".into()),
                                Value::Integer((res.count as i64).into()),
                            ));
                            Value::Map(resmap)
                        })
                        .collect();
                    rmap.push((Value::String("results".into()), Value::Array(result_values)));

                    // Extended recipe fields
                    match &r.station {
                        Some(s) => rmap.push((
                            Value::String("station".into()),
                            Value::String(s.clone().into()),
                        )),
                        None => rmap.push((Value::String("station".into()), Value::Nil)),
                    }
                    rmap.push((
                        Value::String("craft_time_ms".into()),
                        Value::Integer((r.craft_time_ms as i64).into()),
                    ));
                    rmap.push((
                        Value::String("xp".into()),
                        Value::Integer((r.xp as i64).into()),
                    ));
                    rmap.push((
                        Value::String("requires_discovery".into()),
                        Value::Boolean(r.requires_discovery),
                    ));
                    if let Some(ref tool) = r.required_tool {
                        rmap.push((
                            Value::String("required_tool".into()),
                            Value::String(tool.clone().into()),
                        ));
                    }
                    if let Some(ref br) = r.burn_result {
                        rmap.push((
                            Value::String("burn_result".into()),
                            Value::String(br.clone().into()),
                        ));
                    }
                    if let Some(bsl) = r.burn_stop_level {
                        rmap.push((
                            Value::String("burn_stop_level".into()),
                            Value::Integer((bsl as i64).into()),
                        ));
                    }

                    Value::Map(rmap)
                })
                .collect();
            map.push((Value::String("recipes".into()), Value::Array(recipe_values)));
            Value::Map(map)
        }
        ServerMessage::CraftResult {
            success,
            recipe_id,
            error,
            items_gained,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("recipeId".into()),
                Value::String(recipe_id.clone().into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));

            let item_values: Vec<Value> = items_gained
                .iter()
                .map(|item| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("itemId".into()),
                        Value::String(item.item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("count".into()),
                        Value::Integer((item.count as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((
                Value::String("itemsGained".into()),
                Value::Array(item_values),
            ));

            Value::Map(map)
        }
        ServerMessage::ShopOpen { npc_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npc_id".into()),
                Value::String(npc_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::ShopData { npc_id, shop } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));

            let mut shop_map = Vec::new();
            shop_map.push((
                Value::String("shopId".into()),
                Value::String(shop.shop_id.clone().into()),
            ));
            shop_map.push((
                Value::String("displayName".into()),
                Value::String(shop.display_name.clone().into()),
            ));
            shop_map.push((
                Value::String("buyMultiplier".into()),
                Value::F64(shop.buy_multiplier as f64),
            ));
            shop_map.push((
                Value::String("sellMultiplier".into()),
                Value::F64(shop.sell_multiplier as f64),
            ));
            let cat_values: Vec<Value> = shop
                .crafting_categories
                .iter()
                .map(|c| Value::String(c.clone().into()))
                .collect();
            shop_map.push((
                Value::String("craftingCategories".into()),
                Value::Array(cat_values),
            ));
            let station_values: Vec<Value> = shop
                .crafting_stations
                .iter()
                .map(|s| Value::String(s.clone().into()))
                .collect();
            shop_map.push((
                Value::String("craftingStations".into()),
                Value::Array(station_values),
            ));

            let stock_values: Vec<Value> = shop
                .stock
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("itemId".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("price".into()),
                        Value::Integer((s.price as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            shop_map.push((Value::String("stock".into()), Value::Array(stock_values)));

            map.push((Value::String("shop".into()), Value::Map(shop_map)));
            Value::Map(map)
        }
        ServerMessage::ShopResult {
            success,
            action,
            item_id,
            quantity,
            gold_change,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("action".into()),
                Value::String(action.clone().into()),
            ));
            map.push((
                Value::String("itemId".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            map.push((
                Value::String("goldChange".into()),
                Value::Integer((*gold_change as i64).into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::ShopStockUpdate {
            npc_id,
            item_id,
            new_quantity,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("npcId".into()),
                Value::String(npc_id.clone().into()),
            ));
            map.push((
                Value::String("itemId".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("newQuantity".into()),
                Value::Integer((*new_quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::DiscoveredRecipes { recipes } => {
            let mut map = Vec::new();
            let recipe_values: Vec<Value> = recipes
                .iter()
                .map(|r| Value::String(r.clone().into()))
                .collect();
            map.push((Value::String("recipes".into()), Value::Array(recipe_values)));
            Value::Map(map)
        }
        ServerMessage::RecipeDiscovered { recipe_id } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingStarted {
            recipe_id,
            duration_ms,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            map.push((
                Value::String("duration_ms".into()),
                Value::Integer((*duration_ms as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingCancelled { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingCompleted {
            recipe_id,
            items_gained,
            xp_gained,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("recipe_id".into()),
                Value::String(recipe_id.clone().into()),
            ));
            let item_values: Vec<Value> = items_gained
                .iter()
                .map(|(item_id, count)| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("item_id".into()),
                        Value::String(item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("count".into()),
                        Value::Integer((*count as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((
                Value::String("items_gained".into()),
                Value::Array(item_values),
            ));
            map.push((
                Value::String("xp_gained".into()),
                Value::Integer((*xp_gained as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::CraftingBatchProgress { completed, total } => {
            let mut map = Vec::new();
            map.push((
                Value::String("completed".into()),
                Value::Integer((*completed as i64).into()),
            ));
            map.push((
                Value::String("total".into()),
                Value::Integer((*total as i64).into()),
            ));
            Value::Map(map)
        }
        // ===== Slayer System Messages =====
        ServerMessage::TradeRequestReceived {
            requester_id,
            requester_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("requester_id".into()),
                Value::String(requester_id.clone().into()),
            ));
            map.push((
                Value::String("requester_name".into()),
                Value::String(requester_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TradeOpened {
            trade_id,
            partner_id,
            partner_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("trade_id".into()),
                Value::String(trade_id.clone().into()),
            ));
            map.push((
                Value::String("partner_id".into()),
                Value::String(partner_id.clone().into()),
            ));
            map.push((
                Value::String("partner_name".into()),
                Value::String(partner_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TradeOfferUpdate {
            partner_items,
            partner_gold,
            partner_accepted,
        } => {
            let mut map = Vec::new();
            let items: Vec<Value> = partner_items
                .iter()
                .map(|item| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("slot_index".into()),
                        Value::Integer((item.slot_index as i64).into()),
                    ));
                    imap.push((
                        Value::String("item_id".into()),
                        Value::String(item.item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("quantity".into()),
                        Value::Integer((item.quantity as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((Value::String("partner_items".into()), Value::Array(items)));
            map.push((
                Value::String("partner_gold".into()),
                Value::Integer((*partner_gold as i64).into()),
            ));
            map.push((
                Value::String("partner_accepted".into()),
                Value::Boolean(*partner_accepted),
            ));
            Value::Map(map)
        }
        ServerMessage::TradeMyOfferUpdate {
            my_items,
            my_gold,
            my_accepted,
        } => {
            let mut map = Vec::new();
            let items: Vec<Value> = my_items
                .iter()
                .map(|item| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("slot_index".into()),
                        Value::Integer((item.slot_index as i64).into()),
                    ));
                    imap.push((
                        Value::String("item_id".into()),
                        Value::String(item.item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("quantity".into()),
                        Value::Integer((item.quantity as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((Value::String("my_items".into()), Value::Array(items)));
            map.push((
                Value::String("my_gold".into()),
                Value::Integer((*my_gold as i64).into()),
            ));
            map.push((
                Value::String("my_accepted".into()),
                Value::Boolean(*my_accepted),
            ));
            Value::Map(map)
        }
        ServerMessage::TradeCompleted {
            items_received,
            gold_received,
        } => {
            let mut map = Vec::new();
            let items: Vec<Value> = items_received
                .iter()
                .map(|item| {
                    let mut imap = Vec::new();
                    imap.push((
                        Value::String("slot_index".into()),
                        Value::Integer((item.slot_index as i64).into()),
                    ));
                    imap.push((
                        Value::String("item_id".into()),
                        Value::String(item.item_id.clone().into()),
                    ));
                    imap.push((
                        Value::String("quantity".into()),
                        Value::Integer((item.quantity as i64).into()),
                    ));
                    Value::Map(imap)
                })
                .collect();
            map.push((Value::String("items_received".into()), Value::Array(items)));
            map.push((
                Value::String("gold_received".into()),
                Value::Integer((*gold_received as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::TradeCancelled { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }

        // ===== Player Stall System Messages =====
        ServerMessage::StallOpened { name, slots } => {
            let mut map = Vec::new();
            map.push((
                Value::String("name".into()),
                Value::String(name.clone().into()),
            ));
            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("price".into()),
                        Value::Integer((s.price as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            Value::Map(map)
        }
        ServerMessage::StallClosed { reason } => {
            let mut map = Vec::new();
            map.push((
                Value::String("reason".into()),
                Value::String(reason.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::StallUpdate { slots } => {
            let mut map = Vec::new();
            let slot_values: Vec<Value> = slots
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("price".into()),
                        Value::Integer((s.price as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            map.push((Value::String("slots".into()), Value::Array(slot_values)));
            Value::Map(map)
        }
        ServerMessage::StallBrowseData {
            seller_id,
            seller_name,
            stall_name,
            items,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("seller_id".into()),
                Value::String(seller_id.clone().into()),
            ));
            map.push((
                Value::String("seller_name".into()),
                Value::String(seller_name.clone().into()),
            ));
            map.push((
                Value::String("stall_name".into()),
                Value::String(stall_name.clone().into()),
            ));
            let item_values: Vec<Value> = items
                .iter()
                .map(|s| {
                    let mut smap = Vec::new();
                    smap.push((
                        Value::String("slot".into()),
                        Value::Integer((s.slot as i64).into()),
                    ));
                    smap.push((
                        Value::String("item_id".into()),
                        Value::String(s.item_id.clone().into()),
                    ));
                    smap.push((
                        Value::String("quantity".into()),
                        Value::Integer((s.quantity as i64).into()),
                    ));
                    smap.push((
                        Value::String("price".into()),
                        Value::Integer((s.price as i64).into()),
                    ));
                    Value::Map(smap)
                })
                .collect();
            map.push((Value::String("items".into()), Value::Array(item_values)));
            Value::Map(map)
        }
        ServerMessage::StallBuyResult {
            success,
            item_id,
            quantity,
            total_price,
            error,
        } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            map.push((
                Value::String("total_price".into()),
                Value::Integer((*total_price as i64).into()),
            ));
            map.push((
                Value::String("error".into()),
                match error {
                    Some(e) => Value::String(e.clone().into()),
                    None => Value::Nil,
                },
            ));
            Value::Map(map)
        }
        ServerMessage::StallSaleNotification {
            item_id,
            quantity,
            gold_received,
            buyer_name,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("item_id".into()),
                Value::String(item_id.clone().into()),
            ));
            map.push((
                Value::String("quantity".into()),
                Value::Integer((*quantity as i64).into()),
            ));
            map.push((
                Value::String("gold_received".into()),
                Value::Integer((*gold_received as i64).into()),
            ));
            map.push((
                Value::String("buyer_name".into()),
                Value::String(buyer_name.clone().into()),
            ));
            Value::Map(map)
        }
        ServerMessage::StallItemUpdate {
            seller_id,
            stall_slot,
            new_quantity,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("seller_id".into()),
                Value::String(seller_id.clone().into()),
            ));
            map.push((
                Value::String("stall_slot".into()),
                Value::Integer((*stall_slot as i64).into()),
            ));
            map.push((
                Value::String("new_quantity".into()),
                Value::Integer((*new_quantity as i64).into()),
            ));
            Value::Map(map)
        }
        ServerMessage::GrandExchangeData {
            balance,
            decimals,
            offers,
            market,
        } => {
            let mut map = Vec::new();
            map.push((
                Value::String("balance".into()),
                Value::Integer((*balance).into()),
            ));
            map.push((
                Value::String("decimals".into()),
                Value::Integer((*decimals as i64).into()),
            ));
            let offer_values: Vec<Value> = offers
                .iter()
                .map(|o| {
                    let mut omap = Vec::new();
                    omap.push((Value::String("id".into()), Value::Integer(o.id.into())));
                    omap.push((
                        Value::String("side".into()),
                        Value::String(o.side.clone().into()),
                    ));
                    omap.push((
                        Value::String("item_id".into()),
                        Value::String(o.item_id.clone().into()),
                    ));
                    omap.push((Value::String("price".into()), Value::Integer(o.price.into())));
                    omap.push((
                        Value::String("quantity".into()),
                        Value::Integer(o.quantity.into()),
                    ));
                    omap.push((
                        Value::String("remaining".into()),
                        Value::Integer(o.remaining.into()),
                    ));
                    omap.push((
                        Value::String("collect_items".into()),
                        Value::Integer(o.collect_items.into()),
                    ));
                    omap.push((
                        Value::String("status".into()),
                        Value::String(o.status.clone().into()),
                    ));
                    Value::Map(omap)
                })
                .collect();
            map.push((Value::String("offers".into()), Value::Array(offer_values)));

            let market_values: Vec<Value> = market
                .iter()
                .map(|m| {
                    let mut mmap = Vec::new();
                    mmap.push((
                        Value::String("side".into()),
                        Value::String(m.side.clone().into()),
                    ));
                    mmap.push((
                        Value::String("item_id".into()),
                        Value::String(m.item_id.clone().into()),
                    ));
                    mmap.push((Value::String("price".into()), Value::Integer(m.price.into())));
                    mmap.push((
                        Value::String("quantity".into()),
                        Value::Integer(m.quantity.into()),
                    ));
                    Value::Map(mmap)
                })
                .collect();
            map.push((Value::String("market".into()), Value::Array(market_values)));
            Value::Map(map)
        }
        ServerMessage::GeResult { success, message } => {
            let mut map = Vec::new();
            map.push((Value::String("success".into()), Value::Boolean(*success)));
            map.push((
                Value::String("message".into()),
                Value::String(message.clone().into()),
            ));
            Value::Map(map)
        }
        _ => return None,
    };
    Some(value)
}
