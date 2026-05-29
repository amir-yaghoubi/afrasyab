use crate::state::AppState;
use afrasyab_drive::{DriveUploader, DEFAULT_FOLDER_NAME, DRIVE_FILE_SCOPE};
use afrasyab_storage::allowed_users::AllowedUsersRepo;
use afrasyab_storage::drive_settings::DriveSettingsRepo;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::users::UsersRepo;
use afrasyab_storage::DbPool;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use oauth2::basic::BasicClient;
use oauth2::{
    reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/oauth/google", get(google_start))
        .route("/oauth/google/callback", get(google_callback))
        .route("/health", get(health))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct StartQuery {
    state: String,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

fn oauth_client(state: &AppState) -> anyhow::Result<BasicClient> {
    let redirect = format!("{}/oauth/google/callback", state.config.public_base_url);
    Ok(BasicClient::new(
        ClientId::new(state.config.google_client_id.clone()),
        Some(ClientSecret::new(state.config.google_client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect)?))
}

async fn google_start(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StartQuery>,
) -> Result<Response, StatusCode> {
    if query.state.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let known = state
        .oauth_store
        .exists(&query.state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !known {
        return Err(StatusCode::BAD_REQUEST);
    }

    let client = oauth_client(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (auth_url, _) = client
        .authorize_url(|| CsrfToken::new(query.state.clone()))
        .add_scope(Scope::new(DRIVE_FILE_SCOPE.to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .url();

    Ok(Redirect::temporary(auth_url.as_str()).into_response())
}

async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<Html<String>, StatusCode> {
    if query.code.is_empty() || query.state.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let telegram_user_id = state
        .oauth_store
        .take(&query.state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let allowed = AllowedUsersRepo::new(&state.pool)
        .is_allowed(telegram_user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "allowlist check on oauth callback");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }

    let client = oauth_client(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let token = client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "oauth code exchange failed");
            StatusCode::BAD_GATEWAY
        })?;

    let refresh_token = token
        .refresh_token()
        .ok_or_else(|| {
            tracing::error!("Google OAuth response missing refresh_token");
            StatusCode::BAD_GATEWAY
        })?
        .secret()
        .clone();

    let users = UsersRepo::new(&state.pool);
    let user = users
        .get_or_create_by_telegram_id(telegram_user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "load user for oauth callback");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let encrypted = state.cipher.encrypt(refresh_token.as_bytes());
    GoogleCredentialsRepo::new(&state.pool)
        .upsert_encrypted(user.id, &encrypted)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "persist google credentials");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let folder_ok = match provision_user_folder(
        state.drive.as_ref(),
        &state.pool,
        user.id,
        &refresh_token,
    )
    .await
    {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(error = %e, user_id = %user.id, "default folder provisioning failed");
            false
        }
    };

    Ok(Html(success_html(folder_ok)))
}

async fn provision_user_folder<D: DriveUploader + ?Sized>(
    drive: &D,
    pool: &DbPool,
    user_id: Uuid,
    refresh_token: &str,
) -> anyhow::Result<()> {
    let access = drive.refresh_access_token(refresh_token).await?;
    let (folder_id, name) = drive
        .create_folder(&access, DEFAULT_FOLDER_NAME, None)
        .await?;
    DriveSettingsRepo::new(pool)
        .upsert_folder(user_id, &folder_id, Some(&name))
        .await?;
    Ok(())
}

fn success_html(folder_ok: bool) -> String {
    let body = if folder_ok {
        "<p>Google Drive is connected. Uploads will go to the <strong>Afrasyab</strong> folder in your Drive. Return to Telegram and send links or files.</p><p>Use <code>/folder</code> to rename that folder or create a different one.</p>"
    } else {
        "<p>Google Drive is connected, but we could not create your upload folder automatically. Return to Telegram and run <code>/folder</code> to set one up.</p>"
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Afrasyab — Google connected</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 32rem; margin: 4rem auto; line-height: 1.5; }}
    h1 {{ font-size: 1.25rem; }}
  </style>
</head>
<body>
  <h1>Google Drive connected</h1>
  {body}
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn health_returns_ok() {
        let state = test_state().await;
        let app = router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn provision_user_folder_creates_drive_settings() {
        use afrasyab_storage::users::UsersRepo;
        use std::path::Path;

        struct FakeDrive;

        #[async_trait::async_trait]
        impl DriveUploader for FakeDrive {
            async fn refresh_access_token(&self, _: &str) -> anyhow::Result<String> {
                Ok("access-token".into())
            }

            async fn create_folder(
                &self,
                _: &str,
                _: &str,
                _: Option<&str>,
            ) -> anyhow::Result<(String, String)> {
                Ok(("fid".into(), "Afrasyab".into()))
            }

            async fn rename_folder(&self, _: &str, _: &str, _: &str) -> anyhow::Result<String> {
                Ok("Renamed".into())
            }

            async fn upload_file(
                &self,
                _: &str,
                _: &str,
                _: &Path,
                _: &str,
            ) -> anyhow::Result<String> {
                Ok("file-id".into())
            }
        }

        let state = test_state().await;
        let user = UsersRepo::new(&state.pool)
            .get_or_create_by_telegram_id(42)
            .await
            .unwrap();

        provision_user_folder(&FakeDrive, &state.pool, user.id, "refresh")
            .await
            .unwrap();

        let folder = DriveSettingsRepo::new(&state.pool)
            .get_folder(user.id)
            .await
            .unwrap()
            .expect("folder settings");
        assert_eq!(folder.folder_id, "fid");
        assert_eq!(folder.folder_name.as_deref(), Some("Afrasyab"));
    }

    async fn test_state() -> Arc<AppState> {
        use afrasyab_core::{Config, TelegramMode};
        use afrasyab_storage::connect;

        let config = Config {
            telegram_bot_token: "token".into(),
            super_admin_telegram_id: 1,
            google_client_id: "id".into(),
            google_client_secret: "secret".into(),
            public_base_url: "https://example.com".into(),
            token_encryption_key: [1u8; 32],
            database_url: "sqlite::memory:".into(),
            http_bind: "127.0.0.1:0".into(),
            max_playlist_items: 25,
            max_concurrent_jobs: 4,
            telegram_mode: TelegramMode::Polling,
            telegram_webhook_secret: None,
            scratch_thresholds: afrasyab_core::parse_scratch_thresholds_mb(512, 1024),
            max_file_bytes: afrasyab_drive::DEFAULT_MAX_FILE_BYTES,
            drive_upload_chunk_bytes: afrasyab_drive::DEFAULT_UPLOAD_CHUNK_BYTES,
        };
        let pool = connect("sqlite::memory:").await.expect("sqlite pool");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrate");
        AppState::new(config, pool).await.expect("app state")
    }
}
