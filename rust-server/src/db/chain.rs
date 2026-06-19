use super::*;

impl Database {
    pub async fn get_wallet_pubkey_for_account(
        &self,
        account_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT wallet_pubkey FROM accounts WHERE id = ?")
                .bind(account_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(pk,)| pk))
    }

    pub async fn get_chain_balance(&self, account_id: i64) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT chain_balance FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_account_id_by_wallet(
        &self,
        wallet_pubkey: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE wallet_pubkey = ?")
                .bind(wallet_pubkey)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn credit_chain_deposit(
        &self,
        account_id: i64,
        tx_signature: &str,
        amount: i64,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let inserted = sqlx::query(
            "INSERT OR IGNORE INTO chain_transactions (account_id, tx_signature, direction, amount, status)
             VALUES (?, ?, 'deposit', ?, 'confirmed')",
        )
        .bind(account_id)
        .bind(tx_signature)
        .bind(amount)
        .execute(&mut *tx)
        .await?;

        if inserted.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }

        sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
            .bind(amount)
            .bind(account_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(true)
    }

    pub async fn reserve_chain_withdraw(
        &self,
        account_id: i64,
        amount: i64,
    ) -> Result<bool, sqlx::Error> {
        let updated = sqlx::query(
            "UPDATE accounts SET chain_balance = chain_balance - ? WHERE id = ? AND chain_balance >= ?",
        )
        .bind(amount)
        .bind(account_id)
        .bind(amount)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() > 0)
    }

    pub async fn finalize_chain_withdraw(
        &self,
        account_id: i64,
        tx_signature: &str,
        amount: i64,
        success: bool,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let status = if success { "confirmed" } else { "failed" };

        sqlx::query(
            "INSERT INTO chain_transactions (account_id, tx_signature, direction, amount, status)
             VALUES (?, ?, 'withdraw', ?, ?)",
        )
        .bind(account_id)
        .bind(tx_signature)
        .bind(amount)
        .bind(status)
        .execute(&mut *tx)
        .await?;

        if !success {
            sqlx::query("UPDATE accounts SET chain_balance = chain_balance + ? WHERE id = ?")
                .bind(amount)
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_chain_transactions(
        &self,
        account_id: i64,
        limit: i64,
    ) -> Result<Vec<ChainTransactionRow>, sqlx::Error> {
        sqlx::query_as(
            "SELECT tx_signature, direction, amount, status, created_at
             FROM chain_transactions
             WHERE account_id = ?
             ORDER BY id DESC
             LIMIT ?",
        )
        .bind(account_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn chain_tx_exists(&self, tx_signature: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM chain_transactions WHERE tx_signature = ?")
                .bind(tx_signature)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChainTransactionRow {
    pub tx_signature: String,
    pub direction: String,
    pub amount: i64,
    pub status: String,
    pub created_at: String,
}
