mod client;
mod resumable;
mod upload_limits;

pub use client::GoogleDriveClient;
pub use upload_limits::{
    validate_upload_chunk_bytes, DEFAULT_MAX_FILE_BYTES, DEFAULT_UPLOAD_CHUNK_BYTES,
};

use std::path::Path;

pub const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
pub const DEFAULT_FOLDER_NAME: &str = "Afrasyab";

#[async_trait::async_trait]
pub trait DriveUploader: Send + Sync {
    async fn refresh_access_token(&self, refresh_token: &str) -> anyhow::Result<String>;

    async fn create_folder(
        &self,
        access_token: &str,
        name: &str,
        parent_id: Option<&str>,
    ) -> anyhow::Result<(String, String)>;

    async fn rename_folder(
        &self,
        access_token: &str,
        folder_id: &str,
        new_name: &str,
    ) -> anyhow::Result<String>;

    async fn upload_file(
        &self,
        access_token: &str,
        folder_id: &str,
        local_path: &Path,
        filename: &str,
    ) -> anyhow::Result<String>;
}
