use crate::traits::FileDownloader;
use anyhow::Context;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use teloxide::net::Download;
use teloxide::requests::Requester;
use teloxide::Bot;
use tokio::io::AsyncWriteExt;

/// Telegram Bot API file fetcher with a hard byte cap.
pub struct TeloxideTelegramDownloader {
    bot: Bot,
}

impl TeloxideTelegramDownloader {
    pub fn new(bot_token: &str) -> Self {
        Self {
            bot: Bot::new(bot_token),
        }
    }

    pub fn from_bot(bot: Bot) -> Self {
        Self { bot }
    }
}

#[async_trait::async_trait]
impl FileDownloader for TeloxideTelegramDownloader {
    async fn download_direct(&self, _url: &url::Url, _dest: &Path) -> anyhow::Result<PathBuf> {
        anyhow::bail!("TeloxideTelegramDownloader does not support direct HTTP downloads")
    }

    async fn download_telegram_file(
        &self,
        file_id: &str,
        dest: &Path,
        max_bytes: u64,
    ) -> anyhow::Result<PathBuf> {
        let file = self.bot.get_file(file_id).await.context("get_file")?;

        let size = file.size as u64;
        anyhow::ensure!(
            size <= max_bytes,
            "telegram file size {size} exceeds limit {max_bytes}"
        );

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut out = tokio::fs::File::create(dest)
            .await
            .with_context(|| format!("create {}", dest.display()))?;
        let mut downloaded: u64 = 0;

        let mut stream = self.bot.download_file_stream(&file.path);

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("telegram download chunk")?;
            downloaded += chunk.len() as u64;
            anyhow::ensure!(
                downloaded <= max_bytes,
                "telegram download exceeded limit {max_bytes}"
            );
            out.write_all(&chunk).await?;
        }
        out.flush().await?;

        Ok(dest.to_path_buf())
    }
}
