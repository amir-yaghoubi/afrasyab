use crate::ephemeral::{expires_in_secs, purge_expired};
use crate::DbPool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPlaylist {
    pub url: String,
    pub entry_urls: Vec<String>,
    pub telegram_user_id: i64,
    pub telegram_chat_id: i64,
    pub user_id: Uuid,
}

pub struct PendingPlaylistStore {
    pool: Arc<DbPool>,
    ttl_secs: u64,
}

impl PendingPlaylistStore {
    pub fn new(pool: Arc<DbPool>, ttl_secs: u64) -> Self {
        Self { pool, ttl_secs }
    }

    pub async fn put(&self, id: &str, pending: &PendingPlaylist) -> anyhow::Result<()> {
        purge_expired(&self.pool, "pending_playlists").await?;
        let payload = serde_json::to_string(pending)?;
        sqlx::query(
            "INSERT OR REPLACE INTO pending_playlists (id, payload, expires_at) VALUES (?, ?, ?)",
        )
        .bind(id)
        .bind(payload)
        .bind(expires_in_secs(self.ttl_secs))
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn take(&self, id: &str) -> anyhow::Result<Option<PendingPlaylist>> {
        purge_expired(&self.pool, "pending_playlists").await?;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT payload FROM pending_playlists
             WHERE id = ? AND expires_at >= datetime('now')",
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        if let Some((payload,)) = row {
            sqlx::query("DELETE FROM pending_playlists WHERE id = ?")
                .bind(id)
                .execute(self.pool.as_ref())
                .await?;
            Ok(Some(serde_json::from_str(&payload)?))
        } else {
            Ok(None)
        }
    }
}
