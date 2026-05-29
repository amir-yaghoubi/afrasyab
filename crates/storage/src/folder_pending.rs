use crate::ephemeral::{expires_in_secs, purge_expired};
use crate::DbPool;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderPendingMode {
    Rename,
    CreateNew,
}

impl FolderPendingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::CreateNew => "create_new",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "rename" => Some(Self::Rename),
            "create_new" => Some(Self::CreateNew),
            _ => None,
        }
    }
}

pub struct FolderPendingStore {
    pool: Arc<DbPool>,
    ttl_secs: u64,
}

impl FolderPendingStore {
    pub fn new(pool: Arc<DbPool>, ttl_secs: u64) -> Self {
        Self { pool, ttl_secs }
    }

    pub async fn set(&self, telegram_user_id: i64) -> anyhow::Result<()> {
        self.set_with_mode(telegram_user_id, FolderPendingMode::CreateNew)
            .await
    }

    pub async fn set_with_mode(
        &self,
        telegram_user_id: i64,
        mode: FolderPendingMode,
    ) -> anyhow::Result<()> {
        purge_expired(&self.pool, "folder_pending").await?;
        sqlx::query(
            "INSERT OR REPLACE INTO folder_pending (telegram_user_id, mode, expires_at)
             VALUES (?, ?, ?)",
        )
        .bind(telegram_user_id)
        .bind(mode.as_str())
        .bind(expires_in_secs(self.ttl_secs))
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn get_mode(
        &self,
        telegram_user_id: i64,
    ) -> anyhow::Result<Option<FolderPendingMode>> {
        purge_expired(&self.pool, "folder_pending").await?;
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT mode FROM folder_pending
             WHERE telegram_user_id = ? AND expires_at >= datetime('now')",
        )
        .bind(telegram_user_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        Ok(row.and_then(|(s,)| FolderPendingMode::parse(&s)))
    }

    pub async fn is_pending(&self, telegram_user_id: i64) -> anyhow::Result<bool> {
        purge_expired(&self.pool, "folder_pending").await?;
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM folder_pending
             WHERE telegram_user_id = ? AND expires_at >= datetime('now')",
        )
        .bind(telegram_user_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        Ok(row.is_some())
    }

    pub async fn clear(&self, telegram_user_id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM folder_pending WHERE telegram_user_id = ?")
            .bind(telegram_user_id)
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }
}
