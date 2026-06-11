use super::*;

impl Database {
    pub async fn create_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        // Check if friendship already exists in either direction
        let existing = sqlx::query(
            r#"SELECT id FROM friendships
               WHERE (requester_id = ? AND recipient_id = ?)
                  OR (requester_id = ? AND recipient_id = ?)"#,
        )
        .bind(requester_id)
        .bind(recipient_id)
        .bind(recipient_id)
        .bind(requester_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if existing.is_some() {
            return Err("Friend request already exists or you are already friends".to_string());
        }

        sqlx::query(
            "INSERT INTO friendships (requester_id, recipient_id, status) VALUES (?, ?, 'pending')",
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        tracing::info!(
            "Friend request created: {} -> {}",
            requester_id,
            recipient_id
        );
        Ok(())
    }

    pub async fn accept_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        let result = sqlx::query(
            "UPDATE friendships SET status = 'accepted' WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("No pending friend request found".to_string());
        }

        tracing::info!(
            "Friend request accepted: {} <- {}",
            requester_id,
            recipient_id
        );
        Ok(())
    }

    pub async fn decline_friend_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<(), String> {
        let result = sqlx::query(
            "DELETE FROM friendships WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("No pending friend request found".to_string());
        }

        tracing::info!(
            "Friend request declined: {} <- {}",
            requester_id,
            recipient_id
        );
        Ok(())
    }

    pub async fn remove_friend(&self, character_id: i64, friend_id: i64) -> Result<(), String> {
        let result = sqlx::query(
            r#"DELETE FROM friendships
               WHERE status = 'accepted'
                 AND ((requester_id = ? AND recipient_id = ?)
                   OR (requester_id = ? AND recipient_id = ?))"#,
        )
        .bind(character_id)
        .bind(friend_id)
        .bind(friend_id)
        .bind(character_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if result.rows_affected() == 0 {
            return Err("Friendship not found".to_string());
        }

        tracing::info!("Friendship removed: {} <-> {}", character_id, friend_id);
        Ok(())
    }

    pub async fn get_friends_list(
        &self,
        character_id: i64,
    ) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT c.id, c.name FROM characters c
               INNER JOIN friendships f ON
                   (f.requester_id = ? AND f.recipient_id = c.id AND f.status = 'accepted')
                OR (f.recipient_id = ? AND f.requester_id = c.id AND f.status = 'accepted')"#,
        )
        .bind(character_id)
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| (r.get("id"), r.get("name"))).collect())
    }

    pub async fn get_pending_requests(
        &self,
        character_id: i64,
    ) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT c.id, c.name FROM characters c
               INNER JOIN friendships f ON f.requester_id = c.id
               WHERE f.recipient_id = ? AND f.status = 'pending'"#,
        )
        .bind(character_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| (r.get("id"), r.get("name"))).collect())
    }

    pub async fn get_character_id_by_name(&self, name: &str) -> Result<Option<i64>, sqlx::Error> {
        let row = sqlx::query("SELECT id FROM characters WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("id")))
    }

    pub async fn get_character_name_by_id(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query("SELECT name FROM characters WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get("name")))
    }

    pub async fn are_friends(&self, char1_id: i64, char2_id: i64) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id FROM friendships
               WHERE status = 'accepted'
                 AND ((requester_id = ? AND recipient_id = ?)
                   OR (requester_id = ? AND recipient_id = ?))"#,
        )
        .bind(char1_id)
        .bind(char2_id)
        .bind(char2_id)
        .bind(char1_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }

    pub async fn has_pending_request(
        &self,
        requester_id: i64,
        recipient_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id FROM friendships WHERE requester_id = ? AND recipient_id = ? AND status = 'pending'"
        )
        .bind(requester_id)
        .bind(recipient_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.is_some())
    }
}
