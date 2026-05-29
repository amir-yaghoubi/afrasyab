use crate::DbPool;
use afrasyab_domain::types::TelegramUserId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    #[sqlx(try_from = "String")]
    pub id: Uuid,
    pub telegram_user_id: TelegramUserId,
    pub created_at: DateTime<Utc>,
    pub onboarding_complete: bool,
}

pub struct UsersRepo<'a> {
    pool: &'a DbPool,
}

impl<'a> UsersRepo<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_id(&self, user_id: Uuid) -> sqlx::Result<Option<UserRow>> {
        sqlx::query_as(
            "SELECT id, telegram_user_id, created_at, onboarding_complete
             FROM users WHERE id = ?",
        )
        .bind(user_id.to_string())
        .fetch_optional(self.pool)
        .await
    }

    pub async fn get_by_telegram_id(
        &self,
        telegram_user_id: TelegramUserId,
    ) -> sqlx::Result<Option<UserRow>> {
        sqlx::query_as(
            "SELECT id, telegram_user_id, created_at, onboarding_complete
             FROM users WHERE telegram_user_id = ?",
        )
        .bind(telegram_user_id)
        .fetch_optional(self.pool)
        .await
    }

    pub async fn get_or_create_by_telegram_id(
        &self,
        telegram_user_id: TelegramUserId,
    ) -> sqlx::Result<UserRow> {
        if let Some(user) = self.get_by_telegram_id(telegram_user_id).await? {
            return Ok(user);
        }
        let id = Uuid::new_v4();
        sqlx::query_as(
            "INSERT INTO users (id, telegram_user_id) VALUES (?, ?)
             RETURNING id, telegram_user_id, created_at, onboarding_complete",
        )
        .bind(id.to_string())
        .bind(telegram_user_id)
        .fetch_one(self.pool)
        .await
    }

    pub async fn set_onboarding_complete(&self, user_id: Uuid) -> sqlx::Result<()> {
        sqlx::query("UPDATE users SET onboarding_complete = 1 WHERE id = ?")
            .bind(user_id.to_string())
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
