use crate::DbPool;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DriveSettingsRow {
    #[sqlx(try_from = "String")]
    pub user_id: Uuid,
    pub folder_id: String,
    pub folder_name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub struct DriveSettingsRepo<'a> {
    pool: &'a DbPool,
}

impl<'a> DriveSettingsRepo<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_folder(
        &self,
        user_id: Uuid,
        folder_id: &str,
        folder_name: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO drive_settings (user_id, folder_id, folder_name)
             VALUES (?, ?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
               folder_id = excluded.folder_id,
               folder_name = excluded.folder_name,
               updated_at = datetime('now')",
        )
        .bind(user_id.to_string())
        .bind(folder_id)
        .bind(folder_name)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_folder(&self, user_id: Uuid) -> sqlx::Result<Option<DriveSettingsRow>> {
        sqlx::query_as(
            "SELECT user_id, folder_id, folder_name, updated_at
             FROM drive_settings WHERE user_id = ?",
        )
        .bind(user_id.to_string())
        .fetch_optional(self.pool)
        .await
    }
}
