use crate::DriveUploader;
use anyhow::Context;
use oauth2::basic::BasicClient;
use oauth2::{
    reqwest::async_http_client, AuthUrl, ClientId, ClientSecret, RefreshToken, TokenResponse,
    TokenUrl,
};
use std::path::Path;

const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";

pub struct GoogleDriveClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    drive_api_base: String,
    drive_upload_base: String,
    max_file_bytes: u64,
    chunk_bytes: u64,
}

impl GoogleDriveClient {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            drive_api_base: DRIVE_API.to_string(),
            drive_upload_base: DRIVE_UPLOAD.to_string(),
            max_file_bytes: crate::DEFAULT_MAX_FILE_BYTES,
            chunk_bytes: crate::DEFAULT_UPLOAD_CHUNK_BYTES,
        }
    }

    pub fn with_http(
        http: reqwest::Client,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self {
            http,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            drive_api_base: DRIVE_API.to_string(),
            drive_upload_base: DRIVE_UPLOAD.to_string(),
            max_file_bytes: crate::DEFAULT_MAX_FILE_BYTES,
            chunk_bytes: crate::DEFAULT_UPLOAD_CHUNK_BYTES,
        }
    }

    pub fn with_file_limits(
        mut self,
        max_file_bytes: u64,
        chunk_bytes: u64,
    ) -> anyhow::Result<Self> {
        crate::validate_upload_chunk_bytes(chunk_bytes)?;
        self.max_file_bytes = max_file_bytes;
        self.chunk_bytes = chunk_bytes;
        Ok(self)
    }

    pub fn with_api_bases(
        mut self,
        drive_api_base: impl Into<String>,
        drive_upload_base: impl Into<String>,
    ) -> Self {
        self.drive_api_base = drive_api_base.into();
        self.drive_upload_base = drive_upload_base.into();
        self
    }

    pub fn oauth_client(&self) -> BasicClient {
        BasicClient::new(
            ClientId::new(self.client_id.clone()),
            Some(ClientSecret::new(self.client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .expect("valid auth url"),
            Some(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                    .expect("valid token url"),
            ),
        )
    }
}

#[async_trait::async_trait]
impl DriveUploader for GoogleDriveClient {
    async fn refresh_access_token(&self, refresh_token: &str) -> anyhow::Result<String> {
        let client = self.oauth_client();
        let token = client
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
            .request_async(async_http_client)
            .await
            .context("refresh access token")?;

        Ok(token.access_token().secret().clone())
    }

    async fn create_folder(
        &self,
        access_token: &str,
        name: &str,
        parent_id: Option<&str>,
    ) -> anyhow::Result<(String, String)> {
        let mut body = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
        });
        if let Some(parent) = parent_id {
            body["parents"] = serde_json::json!([parent]);
        }

        let url = format!("{}/files", self.drive_api_base);
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .query(&[("fields", "id,name")])
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("create folder request")?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::ensure!(
            status.is_success(),
            "create folder failed ({status}): {text}"
        );

        let meta: serde_json::Value =
            serde_json::from_str(&text).context("parse create folder response")?;
        let id = meta
            .get("id")
            .and_then(|v| v.as_str())
            .context("missing folder id")?;
        let folder_name = meta
            .get("name")
            .and_then(|v| v.as_str())
            .context("missing folder name")?;
        Ok((id.to_string(), folder_name.to_string()))
    }

    async fn rename_folder(
        &self,
        access_token: &str,
        folder_id: &str,
        new_name: &str,
    ) -> anyhow::Result<String> {
        let url = format!("{}/files/{folder_id}", self.drive_api_base);
        let patch_body = serde_json::json!({ "name": new_name }).to_string();
        let response = self
            .http
            .patch(url)
            .bearer_auth(access_token)
            .query(&[("fields", "name")])
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(patch_body)
            .send()
            .await
            .context("rename folder request")?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::ensure!(
            status.is_success(),
            "rename folder failed ({status}): {text}"
        );

        let meta: serde_json::Value =
            serde_json::from_str(&text).context("parse rename response")?;
        meta.get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .context("rename response missing name")
    }

    async fn upload_file(
        &self,
        access_token: &str,
        folder_id: &str,
        local_path: &Path,
        filename: &str,
    ) -> anyhow::Result<String> {
        crate::resumable::upload_resumable(crate::resumable::ResumableUpload {
            http: &self.http,
            drive_upload_base: &self.drive_upload_base,
            access_token,
            folder_id,
            local_path,
            filename,
            max_file_bytes: self.max_file_bytes,
            chunk_bytes: self.chunk_bytes,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DriveUploader;
    #[tokio::test]
    async fn upload_file_resumable_single_chunk() {
        let mut server = mockito::Server::new_async().await;
        let upload_url = format!("{}/session-upload", server.url());
        let _init = server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "resumable".into(),
            ))
            .with_status(200)
            .with_header("location", upload_url.as_str())
            .create_async()
            .await;
        let _put = server
            .mock("PUT", "/session-upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"uploaded-file-id"}"#)
            .create_async()
            .await;

        let base = server.url();
        let client = GoogleDriveClient::new("client-id", "client-secret")
            .with_api_bases(
                format!("{base}/drive/v3"),
                format!("{base}/upload/drive/v3/files"),
            )
            .with_file_limits(1024 * 1024, 256 * 1024)
            .unwrap();

        let tmp = std::env::temp_dir().join(format!("drive-upload-{}", std::process::id()));
        std::fs::write(&tmp, b"payload").unwrap();

        let file_id = client
            .upload_file("token", "folder123", &tmp, "clip.mp4")
            .await
            .unwrap();

        assert_eq!(file_id, "uploaded-file-id");
        let _ = std::fs::remove_file(tmp);
    }

    #[tokio::test]
    async fn upload_file_rejects_over_max_without_http() {
        let client = GoogleDriveClient::new("id", "secret")
            .with_file_limits(10, 256 * 1024)
            .unwrap();
        let tmp = std::env::temp_dir().join(format!("drive-big-{}", std::process::id()));
        std::fs::write(&tmp, vec![0u8; 20]).unwrap();
        let err = client
            .upload_file("t", "f", &tmp, "big.bin")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("exceeds file limit"));
        let _ = std::fs::remove_file(tmp);
    }

    #[tokio::test]
    async fn upload_file_resumable_308_then_completes() {
        let mut server = mockito::Server::new_async().await;
        let upload_url = format!("{}/session", server.url());
        let _init = server
            .mock("POST", "/upload/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "uploadType".into(),
                "resumable".into(),
            ))
            .with_status(200)
            .with_header("location", upload_url.as_str())
            .create_async()
            .await;
        let _put1 = server
            .mock("PUT", "/session")
            .with_status(308)
            .with_header("range", "bytes=0-262143")
            .create_async()
            .await;
        let _put2 = server
            .mock("PUT", "/session")
            .with_status(200)
            .with_body(r#"{"id":"done-id"}"#)
            .create_async()
            .await;

        let base = server.url();
        let client = GoogleDriveClient::new("client-id", "client-secret")
            .with_api_bases(
                format!("{base}/drive/v3"),
                format!("{base}/upload/drive/v3/files"),
            )
            .with_file_limits(1024 * 1024, 256 * 1024)
            .unwrap();

        let tmp = std::env::temp_dir().join(format!("drive-308-{}", std::process::id()));
        std::fs::write(&tmp, vec![0u8; 300_000]).unwrap();

        let file_id = client
            .upload_file("token", "folder123", &tmp, "big.bin")
            .await
            .unwrap();

        assert_eq!(file_id, "done-id");
        let _ = std::fs::remove_file(tmp);
    }

    #[tokio::test]
    async fn create_folder_posts_to_drive_api() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/drive/v3/files")
            .match_query(mockito::Matcher::UrlEncoded(
                "fields".into(),
                "id,name".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"folder-abc","name":"Afrasyab"}"#)
            .create_async()
            .await;

        let base = server.url();
        let client = GoogleDriveClient::new("client-id", "client-secret").with_api_bases(
            format!("{base}/drive/v3"),
            format!("{base}/upload/drive/v3/files"),
        );

        let (id, name) = client
            .create_folder("token", "Afrasyab", None)
            .await
            .unwrap();
        assert_eq!(id, "folder-abc");
        assert_eq!(name, "Afrasyab");
    }

    #[tokio::test]
    async fn rename_folder_patches_drive_api() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("PATCH", "/drive/v3/files/folder-abc")
            .match_query(mockito::Matcher::UrlEncoded("fields".into(), "name".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"name":"My Downloads"}"#)
            .create_async()
            .await;

        let base = server.url();
        let client = GoogleDriveClient::new("client-id", "client-secret").with_api_bases(
            format!("{base}/drive/v3"),
            format!("{base}/upload/drive/v3/files"),
        );

        let name = client
            .rename_folder("token", "folder-abc", "My Downloads")
            .await
            .unwrap();
        assert_eq!(name, "My Downloads");
    }
}
