pub async fn log_runtime_versions() {
    match tokio::process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            tracing::info!(
                version = %String::from_utf8_lossy(&o.stdout).trim(),
                "yt-dlp available"
            );
        }
        _ => tracing::error!("yt-dlp not found or failed --version"),
    }
    match tokio::process::Command::new("deno").arg("--version").output().await {
        Ok(o) if o.status.success() => {
            tracing::info!(
                version = %String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim(),
                "deno available"
            );
        }
        _ => {
            tracing::error!("deno not found — YouTube downloads will fail until image is fixed");
        }
    }
}
