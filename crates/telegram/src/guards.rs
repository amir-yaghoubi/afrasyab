use afrasyab_core::Config;
use afrasyab_storage::allowed_users::AllowedUsersRepo;
use afrasyab_storage::DbPool;

pub fn is_super_admin(user_id: i64, config: &Config) -> bool {
    user_id == config.super_admin_telegram_id
}

pub async fn require_allowed(pool: &DbPool, user_id: i64) -> Result<(), &'static str> {
    let repo = AllowedUsersRepo::new(pool);
    if repo.is_allowed(user_id).await.unwrap_or(false) {
        Ok(())
    } else {
        Err("You don't have access to Afrasyab. Ask the admin.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_admin_matches_config() {
        let config = Config {
            telegram_bot_token: "t".into(),
            super_admin_telegram_id: 99,
            google_client_id: "id".into(),
            google_client_secret: "secret".into(),
            public_base_url: "https://example.com".into(),
            token_encryption_key: [0u8; 32],
            database_url: "sqlite::memory:".into(),
            http_bind: "0.0.0.0:8080".into(),
            max_playlist_items: 25,
            max_concurrent_jobs: 4,
            telegram_mode: afrasyab_core::TelegramMode::Polling,
            telegram_webhook_secret: None,
            scratch_thresholds: afrasyab_core::parse_scratch_thresholds_mb(512, 1024),
            max_file_bytes: afrasyab_drive::DEFAULT_MAX_FILE_BYTES,
            drive_upload_chunk_bytes: afrasyab_drive::DEFAULT_UPLOAD_CHUNK_BYTES,
        };
        assert!(is_super_admin(99, &config));
        assert!(!is_super_admin(100, &config));
    }
}
