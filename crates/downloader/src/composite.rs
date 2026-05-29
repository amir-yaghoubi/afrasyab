use crate::http::ReqwestDownloader;
use crate::telegram::TeloxideTelegramDownloader;
use crate::traits::FileDownloader;
use std::path::{Path, PathBuf};

/// HTTP + Telegram downloads behind one [`FileDownloader`].
pub struct CompositeDownloader {
    http: ReqwestDownloader,
    telegram: TeloxideTelegramDownloader,
}

impl CompositeDownloader {
    pub fn new(bot_token: &str, max_http_bytes: u64) -> Self {
        Self {
            http: ReqwestDownloader::new(max_http_bytes),
            telegram: TeloxideTelegramDownloader::new(bot_token),
        }
    }
}

#[async_trait::async_trait]
impl FileDownloader for CompositeDownloader {
    async fn download_direct(&self, url: &url::Url, dest: &Path) -> anyhow::Result<PathBuf> {
        self.http.download_direct(url, dest).await
    }

    async fn download_telegram_file(
        &self,
        file_id: &str,
        dest: &Path,
        max_bytes: u64,
    ) -> anyhow::Result<PathBuf> {
        self.telegram
            .download_telegram_file(file_id, dest, max_bytes)
            .await
    }
}
