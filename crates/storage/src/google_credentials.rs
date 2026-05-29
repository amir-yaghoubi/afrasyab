use crate::DbPool;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct GoogleCredentialsRepo<'a> {
    pool: &'a DbPool,
}

impl<'a> GoogleCredentialsRepo<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_encrypted(
        &self,
        user_id: Uuid,
        encrypted_refresh_token: &[u8],
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO google_credentials (user_id, encrypted_refresh_token)
             VALUES (?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
               encrypted_refresh_token = excluded.encrypted_refresh_token,
               updated_at = datetime('now')",
        )
        .bind(user_id.to_string())
        .bind(encrypted_refresh_token)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_decrypted_refresh(&self, user_id: Uuid) -> sqlx::Result<Option<Vec<u8>>> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT encrypted_refresh_token FROM google_credentials WHERE user_id = ?",
        )
        .bind(user_id.to_string())
        .fetch_optional(self.pool)
        .await?;
        Ok(row.map(|(token,)| token))
    }

    #[allow(dead_code)]
    pub async fn delete_for_user(&self, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM google_credentials WHERE user_id = ?")
            .bind(user_id.to_string())
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GoogleCredentialsRow {
    #[sqlx(try_from = "String")]
    pub user_id: Uuid,
    pub encrypted_refresh_token: Vec<u8>,
    pub access_token: Option<String>,
    pub access_token_expires_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}
