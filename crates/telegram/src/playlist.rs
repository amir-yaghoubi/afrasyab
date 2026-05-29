use afrasyab_downloader::friendly_ytdlp_message;
use std::time::Duration;

const PLAYLIST_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Returns entry count when the URL is a multi-entry playlist, or `None` for a single video.
pub async fn detect_playlist_size(url: &str) -> anyhow::Result<Option<usize>> {
    let output = tokio::time::timeout(
        PLAYLIST_PROBE_TIMEOUT,
        tokio::process::Command::new("yt-dlp")
            .args(["--flat-playlist", "-J", url])
            .output(),
    )
    .await??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}", friendly_ytdlp_message(&stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let entries = json
        .get("entries")
        .and_then(|e| e.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    if entries <= 1 {
        Ok(None)
    } else {
        Ok(Some(entries))
    }
}

pub fn playlist_entry_urls(json: &serde_json::Value) -> Vec<String> {
    json.get("entries")
        .and_then(|e| e.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("url")
                        .or_else(|| entry.get("webpage_url"))
                        .and_then(|u| u.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn fetch_playlist_entry_urls(url: &str) -> anyhow::Result<Vec<String>> {
    let output = tokio::time::timeout(
        PLAYLIST_PROBE_TIMEOUT,
        tokio::process::Command::new("yt-dlp")
            .args(["--flat-playlist", "-J", url])
            .output(),
    )
    .await??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{}", friendly_ytdlp_message(&stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(playlist_entry_urls(&json))
}
