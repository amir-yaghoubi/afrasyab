use crate::filename::resolve_direct_dest;
use crate::traits::FileDownloader;
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Spec default for direct HTTP downloads.
pub const DIRECT_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Direct HTTP downloader with a byte limit and one retry on transient failures.
pub struct ReqwestDownloader {
    client: reqwest::Client,
    max_bytes: u64,
}

impl ReqwestDownloader {
    pub fn new(max_bytes: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(DIRECT_DOWNLOAD_TIMEOUT)
            .connect_timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { client, max_bytes }
    }

    pub fn with_client(client: reqwest::Client, max_bytes: u64) -> Self {
        Self { client, max_bytes }
    }

    async fn download_once(&self, url: &url::Url, dest: &Path) -> anyhow::Result<PathBuf> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("GET {}", url))?;

        let status = response.status();
        anyhow::ensure!(status.is_success(), "HTTP {status} for {url}");

        if let Some(len) = response.content_length() {
            anyhow::ensure!(
                len <= self.max_bytes,
                "remote file size {len} exceeds limit {}",
                self.max_bytes
            );
        }

        let dest = resolve_direct_dest(dest, response.headers());

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&dest)
            .await
            .with_context(|| format!("create {}", dest.display()))?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("read response body")?;
            downloaded += chunk.len() as u64;
            anyhow::ensure!(
                downloaded <= self.max_bytes,
                "download exceeded size limit {}",
                self.max_bytes
            );
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        Ok(dest.to_path_buf())
    }
}

#[async_trait::async_trait]
impl FileDownloader for ReqwestDownloader {
    async fn download_direct(&self, url: &url::Url, dest: &Path) -> anyhow::Result<PathBuf> {
        match self.download_once(url, dest).await {
            Ok(path) => Ok(path),
            Err(first) => {
                tracing::warn!(error = %first, "direct download failed, retrying once");
                self.download_once(url, dest)
                    .await
                    .with_context(|| format!("retry failed after: {first}"))
            }
        }
    }

    async fn download_telegram_file(
        &self,
        _file_id: &str,
        _dest: &Path,
        _max_bytes: u64,
    ) -> anyhow::Result<PathBuf> {
        anyhow::bail!("ReqwestDownloader does not support Telegram file downloads")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn download_direct_fetches_mocked_url() {
        let mut server = mockito::Server::new_async().await;
        let body = b"hello-mock-download";
        let _mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body(body.as_slice())
            .create_async()
            .await;

        let url = url::Url::parse(&format!("{}/file.bin", server.url())).unwrap();
        let dest = std::env::temp_dir().join(format!("afrasyab-dl-{}.bin", uuid::Uuid::new_v4()));

        let downloader = ReqwestDownloader::new(1024);
        let path = downloader.download_direct(&url, &dest).await.unwrap();
        assert_eq!(path, dest);
        let got = tokio::fs::read(&path).await.unwrap();
        assert_eq!(got, body);
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn download_direct_uses_content_disposition_filename() {
        let mut server = mockito::Server::new_async().await;
        let body = b"pdf-bytes";
        let _mock = server
            .mock("GET", "/dl")
            .with_status(200)
            .with_header(
                "content-disposition",
                "attachment; filename=\"quarterly-report.pdf\"",
            )
            .with_body(body.as_slice())
            .create_async()
            .await;

        let url = url::Url::parse(&format!("{}/dl", server.url())).unwrap();
        let dest = std::env::temp_dir().join(format!("afrasyab-dl-{}", uuid::Uuid::new_v4()));

        let downloader = ReqwestDownloader::new(1024);
        let path = downloader.download_direct(&url, &dest).await.unwrap();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("quarterly-report.pdf"));
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn download_direct_rejects_oversized_content_length() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/big.bin")
            .with_status(200)
            .with_body(vec![0u8; 9999])
            .create_async()
            .await;

        let url = url::Url::parse(&format!("{}/big.bin", server.url())).unwrap();
        let dest = std::env::temp_dir().join(format!("afrasyab-big-{}", uuid::Uuid::new_v4()));

        let downloader = ReqwestDownloader::new(100);
        let err = downloader.download_direct(&url, &dest).await.unwrap_err();
        assert!(err.to_string().contains("exceeds limit"));
    }
}
