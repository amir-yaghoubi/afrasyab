use std::path::{Path, PathBuf};

#[async_trait::async_trait]
pub trait FileDownloader: Send + Sync {
    async fn download_direct(&self, url: &url::Url, dest: &Path) -> anyhow::Result<PathBuf>;

    async fn download_telegram_file(
        &self,
        file_id: &str,
        dest: &Path,
        max_bytes: u64,
    ) -> anyhow::Result<PathBuf>;
}
