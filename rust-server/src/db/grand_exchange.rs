use super::*;

/// A Grand Exchange offer row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GeOffer {
    pub id: i64,
    pub account_id: i64,
    pub character_id: i64,
    pub side: String,
    pub item_id: String,
    pub price: i64,
    pub quantity: i64,
    pub remaining: i64,
    pub collect_items: i64,
    pub status: String,
    pub created_at: String,
}

/// Outcome of placing an offer (for player feedback messaging).
#[derive(Debug, Clone)]
pub struct GePlaceResult {
    pub offer_id: i64,
    /// Units immediately matched against the resting book.
    pub filled: i64,
    /// SOLST base units that changed hands on this player's behalf
    /// (spent by a buyer, earned by a seller).
    pub coins_moved: i64,
}

/// Items returned to the owner's collect box from a cancellation, if any.
#[derive(Debug, Clone)]
pub struct GeCancelResult {
    pub side: String,
    pub item_id: String,
    /// Units returned to the collect box (sell offers) — buy refunds go to balance.
    pub returned_to_collect: i64,
    /// SOLST base units refunded to balance (buy offers).
    pub coins_refunded: i64,
}

impl Database {
    /// Player's own offers (active + awaiting collection), newest first.
    pub async fn ge_offers_for_account(
        &self,
        account_id: i64,
    ) -> Result<Vec<GeOffer>, sqlx::Error> {
        sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE account_id = ?
             ORDER BY id DESC",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Snapshot of the resting book (active offers with units remaining).
    pub async fn ge_active_market(&self, limit: i64) -> Result<Vec<GeOffer>, sqlx::Error> {
        sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE status = 'active' AND remaining > 0
             ORDER BY item_id ASC, side ASC, price DESC, id ASC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch a single offer (ownership checked by caller).
    pub async fn ge_get_offer(&self, offer_id: i64) -> Result<Option<GeOffer>, sqlx::Error> {
        sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE id = ?",
        )
        .bind(offer_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Place a BUY offer. Reserves `price * quantity` SOLST from the account
    /// balance, then matches against the cheapest resting sell offers (paying the
    /// resting price and refunding any price improvement). Returns an error string
    /// if the balance is insufficient.
    pub async fn ge_place_buy(
        &self,
        account_id: i64,
        character_id: i64,
        item_id: &str,
        price: i64,
        quantity: i64,
    ) -> Result<Result<GePlaceResult, String>, sqlx::Error> {
        let cost = match price.checked_mul(quantity) {
            Some(c) if c > 0 => c,
            _ => return Ok(Err("Order total is out of range".to_string())),
        };

        let mut tx = self.pool.begin().await?;

        // Reserve the full notional from the buyer's balance.
        let reserved = sqlx::query(
            "UPDATE accounts SET chain_balance = chain_balance - ?
             WHERE id = ? AND chain_balance >= ?",
        )
        .bind(cost)
        .bind(account_id)
        .bind(cost)
        .execute(&mut *tx)
        .await?;
        if reserved.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(Err("Insufficient SOLST balance".to_string()));
        }

        let offer_id = sqlx::query(
            "INSERT INTO grand_exchange_offers
                (account_id, character_id, side, item_id, price, quantity, remaining, status)
             VALUES (?, ?, 'buy', ?, ?, ?, ?, 'active')",
        )
        .bind(account_id)
        .bind(character_id)
        .bind(item_id)
        .bind(price)
        .bind(quantity)
        .bind(quantity)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();

        // Match against the cheapest sell offers at or below our price.
        let candidates = sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE side = 'sell' AND status = 'active' AND remaining > 0
               AND item_id = ? AND price <= ?
             ORDER BY price ASC, id ASC",
        )
        .bind(item_id)
        .bind(price)
        .fetch_all(&mut *tx)
        .await?;

        let mut buyer_remaining = quantity;
        let mut buyer_collected = 0i64;
        let mut refund = 0i64;
        let mut coins_moved = 0i64;

        for sell in candidates {
            if buyer_remaining == 0 {
                break;
            }
            let m = buyer_remaining.min(sell.remaining);
            if m <= 0 {
                continue;
            }
            let trade_price = sell.price;
            let proceeds = trade_price * m;

            // Pay the seller.
            sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
                .bind(proceeds)
                .bind(sell.account_id)
                .execute(&mut *tx)
                .await?;

            // Shrink the seller offer; delete if fully settled.
            let sell_new = sell.remaining - m;
            finalize_offer(&mut tx, sell.id, sell_new, sell.collect_items).await?;

            buyer_remaining -= m;
            buyer_collected += m;
            refund += (price - trade_price) * m;
            coins_moved += proceeds;
        }

        // Update buyer offer with leftover + collected items.
        sqlx::query(
            "UPDATE grand_exchange_offers
             SET remaining = ?, collect_items = collect_items + ?, status = ?
             WHERE id = ?",
        )
        .bind(buyer_remaining)
        .bind(buyer_collected)
        .bind(if buyer_remaining == 0 { "done" } else { "active" })
        .bind(offer_id)
        .execute(&mut *tx)
        .await?;

        // Refund price improvement (resting sells were cheaper than our bid).
        if refund > 0 {
            sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
                .bind(refund)
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(Ok(GePlaceResult {
            offer_id,
            filled: buyer_collected,
            coins_moved: coins_moved - refund,
        }))
    }

    /// Place a SELL offer. Items must already be escrowed off the seller's
    /// inventory by the caller. Matches against the highest-priced resting buy
    /// offers (selling at the resting buyer's price). Proceeds credit the
    /// seller's SOLST balance immediately.
    pub async fn ge_place_sell(
        &self,
        account_id: i64,
        character_id: i64,
        item_id: &str,
        price: i64,
        quantity: i64,
    ) -> Result<GePlaceResult, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let offer_id = sqlx::query(
            "INSERT INTO grand_exchange_offers
                (account_id, character_id, side, item_id, price, quantity, remaining, status)
             VALUES (?, ?, 'sell', ?, ?, ?, ?, 'active')",
        )
        .bind(account_id)
        .bind(character_id)
        .bind(item_id)
        .bind(price)
        .bind(quantity)
        .bind(quantity)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();

        let candidates = sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE side = 'buy' AND status = 'active' AND remaining > 0
               AND item_id = ? AND price >= ?
             ORDER BY price DESC, id ASC",
        )
        .bind(item_id)
        .bind(price)
        .fetch_all(&mut *tx)
        .await?;

        let mut seller_remaining = quantity;
        let mut proceeds_total = 0i64;
        let mut filled = 0i64;

        for buy in candidates {
            if seller_remaining == 0 {
                break;
            }
            let m = seller_remaining.min(buy.remaining);
            if m <= 0 {
                continue;
            }
            // Buyer already reserved at their own (higher) price, so trade at it.
            let trade_price = buy.price;
            proceeds_total += trade_price * m;

            // Deliver items into the buyer's collect box; shrink their offer.
            let buy_new = buy.remaining - m;
            sqlx::query(
                "UPDATE grand_exchange_offers
                 SET remaining = ?, collect_items = collect_items + ?, status = ?
                 WHERE id = ?",
            )
            .bind(buy_new)
            .bind(m)
            .bind(if buy_new == 0 { "done" } else { "active" })
            .bind(buy.id)
            .execute(&mut *tx)
            .await?;

            seller_remaining -= m;
            filled += m;
        }

        if proceeds_total > 0 {
            sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
                .bind(proceeds_total)
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
        }

        // Update seller offer; delete if fully matched (sells carry no collect box).
        finalize_offer(&mut tx, offer_id, seller_remaining, 0).await?;

        tx.commit().await?;
        Ok(GePlaceResult {
            offer_id,
            filled,
            coins_moved: proceeds_total,
        })
    }

    /// Cancel the resting portion of an offer. Buy offers refund reserved SOLST;
    /// sell offers return unsold items to the collect box. Verifies ownership.
    pub async fn ge_cancel(
        &self,
        account_id: i64,
        offer_id: i64,
    ) -> Result<Result<GeCancelResult, String>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let offer = sqlx::query_as::<_, GeOffer>(
            "SELECT id, account_id, character_id, side, item_id, price, quantity, remaining,
                    collect_items, status, created_at
             FROM grand_exchange_offers
             WHERE id = ?",
        )
        .bind(offer_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(offer) = offer else {
            tx.rollback().await?;
            return Ok(Err("Offer not found".to_string()));
        };
        if offer.account_id != account_id {
            tx.rollback().await?;
            return Ok(Err("That is not your offer".to_string()));
        }
        if offer.remaining <= 0 {
            tx.rollback().await?;
            return Ok(Err("Offer has nothing left to cancel".to_string()));
        }

        let mut result = GeCancelResult {
            side: offer.side.clone(),
            item_id: offer.item_id.clone(),
            returned_to_collect: 0,
            coins_refunded: 0,
        };

        if offer.side == "buy" {
            let refund = offer.price * offer.remaining;
            sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
                .bind(refund)
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
            result.coins_refunded = refund;
            finalize_offer(&mut tx, offer.id, 0, offer.collect_items).await?;
        } else {
            // Return unsold items to the collect box.
            let new_collect = offer.collect_items + offer.remaining;
            result.returned_to_collect = offer.remaining;
            finalize_offer(&mut tx, offer.id, 0, new_collect).await?;
        }

        tx.commit().await?;
        Ok(Ok(result))
    }

    /// Reduce an offer's collect box after the caller has delivered `taken`
    /// units into the player's inventory. Deletes the offer when fully settled.
    pub async fn ge_reduce_collect(
        &self,
        offer_id: i64,
        taken: i64,
    ) -> Result<(), sqlx::Error> {
        if taken <= 0 {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        let row: Option<(i64, i64)> = sqlx::query_as(
            "SELECT remaining, collect_items FROM grand_exchange_offers WHERE id = ?",
        )
        .bind(offer_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((remaining, collect_items)) = row {
            let new_collect = (collect_items - taken).max(0);
            finalize_offer(&mut tx, offer_id, remaining, new_collect).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

/// Apply a new `remaining`/`collect_items` to an offer, deleting it when both
/// reach zero (nothing resting and nothing left to collect), otherwise marking
/// it active/done appropriately.
async fn finalize_offer(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    offer_id: i64,
    remaining: i64,
    collect_items: i64,
) -> Result<(), sqlx::Error> {
    if remaining <= 0 && collect_items <= 0 {
        sqlx::query("DELETE FROM grand_exchange_offers WHERE id = ?")
            .bind(offer_id)
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query(
            "UPDATE grand_exchange_offers
             SET remaining = ?, collect_items = ?, status = ?
             WHERE id = ?",
        )
        .bind(remaining)
        .bind(collect_items)
        .bind(if remaining > 0 { "active" } else { "done" })
        .bind(offer_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
