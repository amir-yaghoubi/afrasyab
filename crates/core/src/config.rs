use anyhow::Context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelegramMode {
    Polling,
    Webhook,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub super_admin_telegram_id: i64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub public_base_url: String,
    pub token_encryption_key: [u8; 32],
    pub database_url: String,
    pub http_bind: String,
    pub max_playlist_items: usize,
    pub max_concurrent_jobs: usize,
    pub telegram_mode: TelegramMode,
    pub telegram_webhook_secret: Option<String>,
    pub scratch_thresholds: crate::scratch_disk::ScratchThresholds,
    pub max_file_bytes: u64,
    pub drive_upload_chunk_bytes: u64,
}

fn parse_u64_env(name: &'static str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(name) {
        Ok(v) => v
            .parse()
            .with_context(|| format!("invalid {name}")),
        Err(_) => Ok(default),
    }
}

fn env_var(name: &'static str) -> anyhow::Result<String> {
    std::env::var(name).with_context(|| {
        format!("missing {name} (set in .env or export it before running afrasyab)")
    })
}

fn parse_telegram_mode() -> anyhow::Result<TelegramMode> {
    match std::env::var("TELEGRAM_MODE")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "polling" => Ok(TelegramMode::Polling),
        "webhook" => Ok(TelegramMode::Webhook),
        other => anyhow::bail!("invalid TELEGRAM_MODE={other:?} (use \"polling\" or \"webhook\")"),
    }
}

fn parse_webhook_secret(mode: TelegramMode) -> anyhow::Result<Option<String>> {
    let secret = std::env::var("TELEGRAM_WEBHOOK_SECRET").unwrap_or_default();
    match mode {
        TelegramMode::Polling => Ok(None),
        TelegramMode::Webhook => {
            anyhow::ensure!(
                !secret.is_empty(),
                "TELEGRAM_WEBHOOK_SECRET is required when TELEGRAM_MODE=webhook"
            );
            anyhow::ensure!(
                !secret.is_empty()
                    && secret.len() <= 256
                    && secret
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "TELEGRAM_WEBHOOK_SECRET must be 1-256 chars from [A-Za-z0-9_-]"
            );
            Ok(Some(secret))
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let key_b64 = env_var("TOKEN_ENCRYPTION_KEY")?;
        let key_bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key_b64.trim())
                .context("TOKEN_ENCRYPTION_KEY must be valid base64")?;
        anyhow::ensure!(
            key_bytes.len() == 32,
            "TOKEN_ENCRYPTION_KEY must be 32 bytes"
        );
        let mut token_encryption_key = [0u8; 32];
        token_encryption_key.copy_from_slice(&key_bytes);
        let public_base_url = env_var("PUBLIC_BASE_URL")?
            .trim_end_matches('/')
            .to_string();
        let telegram_mode = parse_telegram_mode()?;
        let telegram_webhook_secret = parse_webhook_secret(telegram_mode)?;
        let min_mb = parse_u64_env("JOB_SCRATCH_MIN_FREE_MB", 512)?;
        let resume_mb = parse_u64_env("JOB_SCRATCH_RESUME_FREE_MB", 1024)?;
        let scratch_thresholds =
            crate::scratch_disk::parse_scratch_thresholds_mb(min_mb, resume_mb);
        let max_file_bytes =
            parse_u64_env("MAX_FILE_BYTES", afrasyab_drive::DEFAULT_MAX_FILE_BYTES)?;
        anyhow::ensure!(max_file_bytes > 0, "MAX_FILE_BYTES must be greater than 0");
        let drive_upload_chunk_bytes = parse_u64_env(
            "DRIVE_UPLOAD_CHUNK_BYTES",
            afrasyab_drive::DEFAULT_UPLOAD_CHUNK_BYTES,
        )?;
        afrasyab_drive::validate_upload_chunk_bytes(drive_upload_chunk_bytes)?;
        Ok(Self {
            telegram_bot_token: env_var("TELEGRAM_BOT_TOKEN")?,
            super_admin_telegram_id: env_var("SUPER_ADMIN_TELEGRAM_ID")?
                .parse()
                .with_context(|| "SUPER_ADMIN_TELEGRAM_ID must be a numeric Telegram user id")?,
            google_client_id: env_var("GOOGLE_CLIENT_ID")?,
            google_client_secret: env_var("GOOGLE_CLIENT_SECRET")?,
            public_base_url,
            token_encryption_key,
            database_url: env_var("DATABASE_URL")?,
            http_bind: std::env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            max_playlist_items: std::env::var("MAX_PLAYLIST_ITEMS")
                .unwrap_or_else(|_| "25".into())
                .parse()?,
            max_concurrent_jobs: std::env::var("MAX_CONCURRENT_JOBS")
                .unwrap_or_else(|_| "4".into())
                .parse()?,
            telegram_mode,
            telegram_webhook_secret,
            scratch_thresholds,
            max_file_bytes,
            drive_upload_chunk_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env test lock")
    }

    fn set_env(key: &str, value: &str) {
        // SAFETY: tests hold env_lock() so no concurrent env mutation.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: tests hold env_lock() so no concurrent env mutation.
        unsafe { std::env::remove_var(key) };
    }

    fn set_required_env(key_b64: &str) {
        set_env("TELEGRAM_BOT_TOKEN", "test-token");
        set_env("SUPER_ADMIN_TELEGRAM_ID", "42");
        set_env("GOOGLE_CLIENT_ID", "client-id");
        set_env("GOOGLE_CLIENT_SECRET", "client-secret");
        set_env("PUBLIC_BASE_URL", "https://example.com");
        set_env("TOKEN_ENCRYPTION_KEY", key_b64);
        set_env("DATABASE_URL", "sqlite::memory:");
    }

    fn cleanup_required_env() {
        for key in [
            "TELEGRAM_BOT_TOKEN",
            "SUPER_ADMIN_TELEGRAM_ID",
            "GOOGLE_CLIENT_ID",
            "GOOGLE_CLIENT_SECRET",
            "PUBLIC_BASE_URL",
            "TOKEN_ENCRYPTION_KEY",
            "DATABASE_URL",
            "TELEGRAM_MODE",
            "TELEGRAM_WEBHOOK_SECRET",
            "HTTP_BIND",
            "MAX_PLAYLIST_ITEMS",
            "MAX_CONCURRENT_JOBS",
            "JOB_SCRATCH_MIN_FREE_MB",
            "JOB_SCRATCH_RESUME_FREE_MB",
            "MAX_FILE_BYTES",
            "DRIVE_UPLOAD_CHUNK_BYTES",
        ] {
            remove_env(key);
        }
    }

    #[test]
    fn from_env_parses_required_and_defaults() {
        let _guard = env_lock();
        let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);

        set_required_env(&key);
        remove_env("HTTP_BIND");
        remove_env("MAX_PLAYLIST_ITEMS");
        remove_env("MAX_CONCURRENT_JOBS");
        remove_env("TELEGRAM_MODE");
        remove_env("TELEGRAM_WEBHOOK_SECRET");

        let config = Config::from_env().expect("config should parse");

        assert_eq!(config.telegram_bot_token, "test-token");
        assert_eq!(config.super_admin_telegram_id, 42);
        assert_eq!(config.public_base_url, "https://example.com");
        assert_eq!(config.token_encryption_key, [7u8; 32]);
        assert_eq!(config.http_bind, "0.0.0.0:8080");
        assert_eq!(config.max_playlist_items, 25);
        assert_eq!(config.max_concurrent_jobs, 4);
        assert!(matches!(config.telegram_mode, TelegramMode::Polling));
        assert!(config.telegram_webhook_secret.is_none());
        assert_eq!(
            config.scratch_thresholds.min_free_bytes,
            512 * 1024 * 1024
        );

        cleanup_required_env();
    }

    #[test]
    fn scratch_threshold_defaults() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        remove_env("JOB_SCRATCH_MIN_FREE_MB");
        remove_env("JOB_SCRATCH_RESUME_FREE_MB");
        let config = Config::from_env().expect("config");
        assert_eq!(
            config.scratch_thresholds.min_free_bytes,
            512 * 1024 * 1024
        );
        cleanup_required_env();
    }

    #[test]
    fn from_env_rejects_wrong_key_length() {
        let _guard = env_lock();
        let short_key = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);

        set_required_env(&short_key);

        let err = Config::from_env().unwrap_err();
        assert!(
            err.to_string().contains("32 bytes"),
            "unexpected error: {err}"
        );

        cleanup_required_env();
    }

    #[test]
    fn telegram_mode_defaults_to_polling() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        remove_env("TELEGRAM_MODE");
        remove_env("TELEGRAM_WEBHOOK_SECRET");

        let config = Config::from_env().expect("config");
        assert!(matches!(config.telegram_mode, TelegramMode::Polling));
        assert!(config.telegram_webhook_secret.is_none());

        cleanup_required_env();
    }

    #[test]
    fn telegram_mode_webhook_requires_secret() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        set_env("TELEGRAM_MODE", "webhook");
        remove_env("TELEGRAM_WEBHOOK_SECRET");

        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("TELEGRAM_WEBHOOK_SECRET"), "{err}");

        cleanup_required_env();
    }

    #[test]
    fn telegram_mode_webhook_rejects_invalid_secret() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        set_env("TELEGRAM_MODE", "webhook");
        set_env("TELEGRAM_WEBHOOK_SECRET", "bad secret!");

        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("TELEGRAM_WEBHOOK_SECRET"), "{err}");

        cleanup_required_env();
    }

    #[test]
    fn telegram_mode_webhook_accepts_valid_secret() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        set_env("TELEGRAM_MODE", "webhook");
        set_env("TELEGRAM_WEBHOOK_SECRET", "aB3-_valid");

        let config = Config::from_env().expect("config");
        assert!(matches!(config.telegram_mode, TelegramMode::Webhook));
        assert_eq!(
            config.telegram_webhook_secret.as_deref(),
            Some("aB3-_valid")
        );

        cleanup_required_env();
    }

    #[test]
    fn drive_upload_chunk_must_be_valid_multiple() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        set_env("DRIVE_UPLOAD_CHUNK_BYTES", "100000");

        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("DRIVE_UPLOAD_CHUNK_BYTES"));

        cleanup_required_env();
    }

    #[test]
    fn telegram_mode_rejects_unknown() {
        let _guard = env_lock();
        set_required_env(&base64::engine::general_purpose::STANDARD.encode([7u8; 32]));
        set_env("TELEGRAM_MODE", "socket");

        let err = Config::from_env().unwrap_err();
        assert!(err.to_string().contains("TELEGRAM_MODE"), "{err}");

        cleanup_required_env();
    }
}
