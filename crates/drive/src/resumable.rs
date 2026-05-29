use anyhow::Context;
use reqwest::Client;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const CHUNK_PUT_RETRIES: u32 = 3;

enum PutOutcome {
    Done(String),
    ResumeFrom(u64),
}

pub struct ResumableUpload<'a> {
    pub http: &'a Client,
    pub drive_upload_base: &'a str,
    pub access_token: &'a str,
    pub folder_id: &'a str,
    pub local_path: &'a Path,
    pub filename: &'a str,
    pub max_file_bytes: u64,
    pub chunk_bytes: u64,
}

pub async fn upload_resumable(params: ResumableUpload<'_>) -> anyhow::Result<String> {
    let ResumableUpload {
        http,
        drive_upload_base,
        access_token,
        folder_id,
        local_path,
        filename,
        max_file_bytes,
        chunk_bytes,
    } = params;

    let meta = tokio::fs::metadata(local_path)
        .await
        .with_context(|| format!("stat {}", local_path.display()))?;
    let total = meta.len();
    anyhow::ensure!(
        total <= max_file_bytes,
        "file size {total} exceeds file limit {max_file_bytes}"
    );

    let session_url = start_resumable_session(
        http,
        drive_upload_base,
        access_token,
        folder_id,
        filename,
        total,
    )
    .await?;

    upload_chunks(
        http,
        &session_url,
        local_path,
        total,
        chunk_bytes as usize,
    )
    .await
}

async fn start_resumable_session(
    http: &Client,
    drive_upload_base: &str,
    access_token: &str,
    folder_id: &str,
    filename: &str,
    total: u64,
) -> anyhow::Result<String> {
    let metadata = serde_json::json!({
        "name": filename,
        "parents": [folder_id],
    });
    let url = format!("{drive_upload_base}?uploadType=resumable&fields=id");
    let response = http
        .post(url)
        .bearer_auth(access_token)
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Type", "application/octet-stream")
        .header("X-Upload-Content-Length", total.to_string())
        .body(metadata.to_string())
        .send()
        .await
        .context("drive resumable session request")?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("drive resumable session failed ({status}): {text}");
    }
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .context("resumable session missing Location header")
}

async fn upload_chunks(
    http: &Client,
    session_url: &str,
    local_path: &Path,
    total: u64,
    chunk_size: usize,
) -> anyhow::Result<String> {
    let mut file = tokio::fs::File::open(local_path)
        .await
        .with_context(|| format!("open {}", local_path.display()))?;
    let mut buf = vec![0u8; chunk_size];
    let mut offset: u64 = 0;

    while offset < total {
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let n = file.read(&mut buf).await.context("read upload chunk")?;
        if n == 0 {
            anyhow::bail!("drive resumable upload stalled at offset {offset}/{total}");
        }
        let chunk = &buf[..n];
        let start = offset;
        let end = offset + n as u64 - 1;

        match put_chunk_with_retries(http, session_url, chunk, start, end, total).await? {
            PutOutcome::Done(id) => return Ok(id),
            PutOutcome::ResumeFrom(next) => offset = next,
        }
    }

    anyhow::bail!("drive resumable upload ended without file id");
}

async fn put_chunk_with_retries(
    http: &Client,
    session_url: &str,
    chunk: &[u8],
    start: u64,
    end: u64,
    total: u64,
) -> anyhow::Result<PutOutcome> {
    let content_range = format!("bytes {start}-{end}/{total}");
    let mut attempt = 0u32;
    loop {
        let response = http
            .put(session_url)
            .header("Content-Length", chunk.len().to_string())
            .header("Content-Range", &content_range)
            .body(chunk.to_vec())
            .send()
            .await
            .context("drive chunk PUT")?;

        let status = response.status();
        if status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let parsed: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            let id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .context("upload response missing file id")?;
            return Ok(PutOutcome::Done(id));
        }
        if status.as_u16() == 308 {
            let next = response
                .headers()
                .get(reqwest::header::RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_bytes_range_end)
                .map(|e| e + 1)
                .unwrap_or(end + 1);
            return Ok(PutOutcome::ResumeFrom(next));
        }
        let retryable = status.is_server_error() || status.as_u16() == 429;
        if retryable && attempt < CHUNK_PUT_RETRIES {
            attempt += 1;
            tokio::time::sleep(std::time::Duration::from_secs(
                1u64 << attempt.min(2),
            ))
            .await;
            continue;
        }
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("drive chunk upload failed ({status}): {text}");
    }
}

fn parse_bytes_range_end(range: &str) -> Option<u64> {
    let rest = range.strip_prefix("bytes=")?;
    let (_start, end) = rest.split_once('-')?;
    end.parse().ok()
}
