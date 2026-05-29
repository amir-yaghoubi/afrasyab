use crate::ephemeral::{expires_in_secs, purge_expired};
use crate::DbPool;
use std::sync::Arc;

pub struct OAuthStateStore {
    pool: Arc<DbPool>,
    ttl_secs: u64,
}

impl OAuthStateStore {
    pub fn new(pool: Arc<DbPool>, ttl_secs: u64) -> Self {
        Self { pool, ttl_secs }
    }

    pub async fn put(&self, state: &str, telegram_user_id: i64) -> sqlx::Result<()> {
        purge_expired(&self.pool, "oauth_states").await?;
        sqlx::query(
            "INSERT OR REPLACE INTO oauth_states (state, telegram_user_id, expires_at)
             VALUES (?, ?, ?)",
        )
        .bind(state)
        .bind(telegram_user_id)
        .bind(expires_in_secs(self.ttl_secs))
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn exists(&self, state: &str) -> sqlx::Result<bool> {
        purge_expired(&self.pool, "oauth_states").await?;
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM oauth_states WHERE state = ? AND expires_at >= datetime('now')",
        )
        .bind(state)
        .fetch_optional(self.pool.as_ref())
        .await?;
        Ok(row.is_some())
    }

    pub async fn take(&self, state: &str) -> sqlx::Result<Option<i64>> {
        purge_expired(&self.pool, "oauth_states").await?;
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT telegram_user_id FROM oauth_states
             WHERE state = ? AND expires_at >= datetime('now')",
        )
        .bind(state)
        .fetch_optional(self.pool.as_ref())
        .await?;
        if let Some((telegram_user_id,)) = row {
            sqlx::query("DELETE FROM oauth_states WHERE state = ?")
                .bind(state)
                .execute(self.pool.as_ref())
                .await?;
            Ok(Some(telegram_user_id))
        } else {
            Ok(None)
        }
    }
}
