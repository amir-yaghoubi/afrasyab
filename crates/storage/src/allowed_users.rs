use crate::DbPool;
use afrasyab_domain::types::TelegramUserId;

pub struct AllowedUsersRepo<'a> {
    pool: &'a DbPool,
}

impl<'a> AllowedUsersRepo<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn is_allowed(&self, telegram_user_id: TelegramUserId) -> sqlx::Result<bool> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM allowed_users WHERE telegram_user_id = ?")
                .bind(telegram_user_id)
                .fetch_optional(self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn add(
        &self,
        telegram_user_id: TelegramUserId,
        added_by: TelegramUserId,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO allowed_users (telegram_user_id, added_by) VALUES (?, ?)
             ON CONFLICT (telegram_user_id) DO NOTHING",
        )
        .bind(telegram_user_id)
        .bind(added_by)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove(&self, telegram_user_id: TelegramUserId) -> sqlx::Result<()> {
        sqlx::query("DELETE FROM allowed_users WHERE telegram_user_id = ?")
            .bind(telegram_user_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn list(&self) -> sqlx::Result<Vec<TelegramUserId>> {
        let rows: Vec<(TelegramUserId,)> =
            sqlx::query_as("SELECT telegram_user_id FROM allowed_users ORDER BY added_at")
                .fetch_all(self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
