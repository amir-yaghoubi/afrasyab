use afrasyab_core::job_log::{job_error_message, job_event, JobPhase};
use afrasyab_core::scratch_disk::disk_full_in_message;
use afrasyab_core::AppState;
use afrasyab_domain::job::transition;
use afrasyab_domain::types::{JobStatus, SourceType, YtDlpFormat};
use afrasyab_downloader::{
    build_args, failure_kind_slug, filename_from_url_path, run_ytdlp, sanitize_filename,
    user_message_for_kind, FileDownloader, YtDlpFailureKind, YtDlpRunError,
};
use afrasyab_drive::DriveUploader;
use afrasyab_storage::drive_settings::DriveSettingsRepo;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::jobs::JobsRepo;
use afrasyab_storage::users::UsersRepo;
use afrasyab_telegram::status::status_text;
use afrasyab_telegram::TelegramNotifier;
use anyhow::Context;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const TELEGRAM_MAX_BYTES: u64 = 20 * 1024 * 1024;
const YTDLP_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub struct JobRunner<D, G, N>
where
    D: FileDownloader,
    G: DriveUploader,
    N: TelegramNotifier + ?Sized,
{
    pub downloader: Arc<D>,
    pub drive: Arc<G>,
    pub notifier: Arc<N>,
    pub state: Arc<AppState>,
}

impl<D, G, N> JobRunner<D, G, N>
where
    D: FileDownloader,
    G: DriveUploader,
    N: TelegramNotifier + ?Sized,
{
    pub async fn run_job(&self, job_id: Uuid) -> anyhow::Result<()> {
        let temp_dir = std::env::temp_dir()
            .join("afrasyab")
            .join(job_id.to_string());
        let cleanup = TempDirGuard::new(temp_dir.clone());

        let result = self.run_job_inner(job_id, &temp_dir).await;
        if let Err(e) = &result {
            if disk_full_in_message(&e.to_string()) {
                self.state.disk_pressure.force_pressure();
            }
            let kind = failure_kind_from_error(e);
            let _ = self
                .fail_job(job_id, &user_facing_error(e), kind)
                .await;
        }
        drop(cleanup);
        result
    }

    async fn run_job_inner(&self, job_id: Uuid, temp_dir: &PathBuf) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(temp_dir).await?;

        let jobs = JobsRepo::new(&self.state.pool);
        let job = jobs.get_by_id(job_id).await?.context("job not found")?;

        let source_type = job
            .source_type()
            .context("unknown source_type in database")?;
        job_event(job_id, source_type, JobPhase::Downloading);

        if self.state.disk_pressure.is_pressured() {
            return Err(anyhow::anyhow!(user_message_for_kind(
                YtDlpFailureKind::DiskFull,
            )));
        }

        let _user = UsersRepo::new(&self.state.pool)
            .get_by_id(job.user_id)
            .await?
            .context("user not found")?;

        let folder = DriveSettingsRepo::new(&self.state.pool)
            .get_folder(job.user_id)
            .await?
            .context("drive folder not configured")?;

        let encrypted = GoogleCredentialsRepo::new(&self.state.pool)
            .get_decrypted_refresh(job.user_id)
            .await?
            .context("google credentials missing")?;

        let refresh_bytes = self
            .state
            .cipher
            .decrypt(&encrypted)
            .map_err(|_| anyhow::anyhow!("google credentials invalid"))?;
        let refresh_token =
            String::from_utf8(refresh_bytes).context("refresh token is not valid utf-8")?;

        let access_token = self
            .state
            .drive
            .refresh_access_token(&refresh_token)
            .await
            .context("refresh access token")?;

        self.set_status(job_id, JobStatus::Downloading, None, None)
            .await?;

        let local_path = match source_type {
            SourceType::LinkDirect => {
                let url = job
                    .source_meta
                    .get("url")
                    .and_then(|v| v.as_str())
                    .context("direct job missing url")?;
                let parsed = url::Url::parse(url)?;
                let name = filename_from_url_path(&parsed).unwrap_or_else(|| {
                    format!("file_{}", &job_id.to_string()[..8])
                });
                let dest = temp_dir.join(sanitize_filename(&name));
                self.downloader.download_direct(&parsed, &dest).await?
            }
            SourceType::LinkYtDlp | SourceType::PlaylistItem => {
                let url = job
                    .source_meta
                    .get("url")
                    .and_then(|v| v.as_str())
                    .context("yt-dlp job missing url")?;
                let format = parse_ytdlp_format(
                    job.source_meta
                        .get("format")
                        .and_then(|v| v.as_str())
                        .unwrap_or("best"),
                );
                let args = build_args(url, temp_dir, format, self.state.config.max_file_bytes);
                run_ytdlp(args, YTDLP_TIMEOUT).await?
            }
            SourceType::TelegramFile => {
                let file_id = job
                    .source_meta
                    .get("file_id")
                    .and_then(|v| v.as_str())
                    .context("telegram job missing file_id")?;
                let name = telegram_local_filename(&job.source_meta, job_id);
                let dest = temp_dir.join(sanitize_filename(&name));
                self.downloader
                    .download_telegram_file(file_id, &dest, TELEGRAM_MAX_BYTES)
                    .await?
            }
        };

        self.set_status(job_id, JobStatus::Uploading, None, None)
            .await?;

        let filename = local_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload.bin")
            .to_string();

        let drive_file_id = self
            .drive
            .upload_file(&access_token, &folder.folder_id, &local_path, &filename)
            .await?;

        jobs.set_drive_file_id(job_id, &drive_file_id).await?;
        self.set_status(job_id, JobStatus::Completed, None, None)
            .await?;
        Ok(())
    }

    async fn set_status(
        &self,
        job_id: Uuid,
        to: JobStatus,
        error: Option<&str>,
        failure_kind: Option<&str>,
    ) -> anyhow::Result<()> {
        let jobs = JobsRepo::new(&self.state.pool);
        let job = jobs
            .get_by_id(job_id)
            .await?
            .context("job not found for status update")?;
        let source_type = job
            .source_type()
            .context("unknown source_type in database")?;
        let from = job.status().unwrap_or(JobStatus::Queued);
        if transition(from, to).is_err() && !matches!(to, JobStatus::Failed) {
            return Ok(());
        }
        jobs.update_status(job_id, to, error).await?;

        match to {
            JobStatus::Uploading => job_event(job_id, source_type, JobPhase::Uploading),
            JobStatus::Completed => job_event(job_id, source_type, JobPhase::Completed),
            JobStatus::Failed => {
                job_event(job_id, source_type, JobPhase::Failed);
                if let Some(msg) = error {
                    job_error_message(job_id, source_type, msg, failure_kind);
                }
            }
            JobStatus::Queued | JobStatus::Downloading => {}
        }

        if let Some(message_id) = job.status_message_id {
            let updated = jobs.get_by_id(job_id).await?.context("reload job")?;
            let text = status_text(&updated);
            let _ = self
                .notifier
                .edit_message(job.telegram_chat_id, message_id, &text)
                .await;
        }
        Ok(())
    }

    async fn fail_job(
        &self,
        job_id: Uuid,
        message: &str,
        failure_kind: Option<&'static str>,
    ) -> anyhow::Result<()> {
        self.set_status(job_id, JobStatus::Failed, Some(message), failure_kind)
            .await
    }
}

fn failure_kind_from_error(err: &anyhow::Error) -> Option<&'static str> {
    if let Some(run) = err
        .chain()
        .find_map(|e| e.downcast_ref::<YtDlpRunError>())
    {
        return Some(failure_kind_slug(run.kind));
    }
    if disk_full_in_message(&err.to_string()) {
        return Some(failure_kind_slug(YtDlpFailureKind::DiskFull));
    }
    None
}

fn user_facing_error(err: &anyhow::Error) -> String {
    if let Some(run) = err.chain().find_map(|e| e.downcast_ref::<YtDlpRunError>()) {
        return user_message_for_kind(run.kind).to_string();
    }
    let msg = err.to_string();
    let lower = msg.to_ascii_lowercase();
    if disk_full_in_message(&lower) {
        return user_message_for_kind(YtDlpFailureKind::DiskFull).to_string();
    }
    if lower.contains("google credentials")
        || lower.contains("refresh token")
        || lower.contains("refresh access")
        || lower.contains("oauth")
    {
        return "Google connection missing or expired. Run /connect.".to_string();
    }
    if lower.contains("drive folder") || lower.contains("folder not configured") {
        return "Drive folder not set. Run /folder.".to_string();
    }
    if lower.contains("yt-dlp") {
        return user_message_for_kind(YtDlpFailureKind::Other).to_string();
    }
    if lower.contains("http") || lower.contains("get ") {
        return "Download failed.".to_string();
    }
    if lower.contains("telegram") || lower.contains("file is too big") || lower.contains("20") {
        return "Telegram file is too large (max 20 MB for bots). Send a direct link instead."
            .to_string();
    }
    if lower.contains("exceeds file limit")
        || lower.contains("exceeded size limit")
        || lower.contains("download exceeded size limit")
    {
        return "File is too large (max 2 GB).".to_string();
    }
    if lower.contains("upload") || lower.contains("drive") {
        return format!("Google Drive upload failed: {}", err);
    }
    format!("Job failed: {}", err)
}

fn parse_ytdlp_format(raw: &str) -> YtDlpFormat {
    match raw {
        "video" => YtDlpFormat::Video,
        "audio" => YtDlpFormat::Audio,
        _ => YtDlpFormat::Best,
    }
}

fn telegram_local_filename(source_meta: &serde_json::Value, job_id: Uuid) -> String {
    if let Some(name) = source_meta.get("file_name").and_then(|v| v.as_str()) {
        if !name.trim().is_empty() {
            return name.to_string();
        }
    }
    format!("file_{}", &job_id.to_string()[..8])
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let path = self.path.clone();
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
#[allow(dead_code)] // Mocks reserved for future job_runner integration tests
mod tests {
    use super::*;
    use afrasyab_domain::types::JobStatus;
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Mutex;

    struct MockDownloader;
    struct MockDrive;
    struct MockNotifier {
        edits: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl FileDownloader for MockDownloader {
        async fn download_direct(&self, _url: &url::Url, dest: &Path) -> anyhow::Result<PathBuf> {
            tokio::fs::write(dest, b"data").await?;
            Ok(dest.to_path_buf())
        }

        async fn download_telegram_file(
            &self,
            _file_id: &str,
            dest: &Path,
            _max: u64,
        ) -> anyhow::Result<PathBuf> {
            tokio::fs::write(dest, b"tg").await?;
            Ok(dest.to_path_buf())
        }
    }

    #[async_trait]
    impl DriveUploader for MockDrive {
        async fn refresh_access_token(&self, _: &str) -> anyhow::Result<String> {
            Ok("access".into())
        }

        async fn create_folder(
            &self,
            _: &str,
            name: &str,
            _: Option<&str>,
        ) -> anyhow::Result<(String, String)> {
            Ok(("mock-folder".into(), name.to_string()))
        }

        async fn rename_folder(&self, _: &str, _: &str, new_name: &str) -> anyhow::Result<String> {
            Ok(new_name.to_string())
        }

        async fn upload_file(&self, _: &str, _: &str, _: &Path, _: &str) -> anyhow::Result<String> {
            Ok("drive-id".into())
        }
    }

    #[async_trait]
    impl TelegramNotifier for MockNotifier {
        async fn send_message(&self, _: i64, _: &str) -> anyhow::Result<i64> {
            Ok(1)
        }

        async fn edit_message(&self, _: i64, _: i64, text: &str) -> anyhow::Result<()> {
            self.edits.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    #[test]
    fn telegram_local_filename_uses_job_id_when_name_missing() {
        let job_id = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        let meta = serde_json::json!({});
        assert_eq!(telegram_local_filename(&meta, job_id), "file_a1b2c3d4");
    }

    #[test]
    fn telegram_local_filename_keeps_provided_name() {
        let job_id = Uuid::new_v4();
        let meta = serde_json::json!({ "file_name": "photo.jpg" });
        assert_eq!(telegram_local_filename(&meta, job_id), "photo.jpg");
    }

    #[test]
    fn parse_format_keys() {
        assert!(matches!(parse_ytdlp_format("video"), YtDlpFormat::Video));
        assert!(matches!(parse_ytdlp_format("audio"), YtDlpFormat::Audio));
        assert!(matches!(parse_ytdlp_format("best"), YtDlpFormat::Best));
    }

    #[test]
    fn status_transition_allows_failure_from_queued() {
        let err = transition(JobStatus::Queued, JobStatus::Failed);
        assert!(err.is_ok());
    }

    #[test]
    fn user_facing_upload_limit_without_path() {
        let err: anyhow::Error =
            anyhow::anyhow!("file size 3000000000 exceeds file limit 2147483648");
        let msg = user_facing_error(&err);
        assert!(msg.contains("too large"));
        assert!(msg.contains("2 GB"));
        assert!(!msg.contains("/tmp"));
    }

    #[test]
    fn user_facing_ytdlp_uses_kind_not_stderr() {
        let err: anyhow::Error = YtDlpRunError {
            kind: YtDlpFailureKind::JsRuntime,
            stderr: "WARNING: No supported JavaScript runtime".into(),
        }
        .into();
        let msg = user_facing_error(&err);
        assert!(!msg.contains("JavaScript"));
        assert!(msg.contains("server update"));
    }
}
