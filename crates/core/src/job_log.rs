use afrasyab_domain::types::SourceType;
use uuid::Uuid;

use crate::sanitize::sanitize_for_log;

#[derive(Debug, Clone, Copy)]
pub enum JobPhase {
    Queued,
    Claimed,
    Downloading,
    Uploading,
    Completed,
    Failed,
}

impl JobPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Downloading => "downloading",
            Self::Uploading => "uploading",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

pub fn source_type_slug(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::LinkYtDlp => "link_yt_dlp",
        SourceType::LinkDirect => "link_direct",
        SourceType::TelegramFile => "telegram_file",
        SourceType::PlaylistItem => "playlist_item",
    }
}

pub fn job_event(job_id: Uuid, source_type: SourceType, phase: JobPhase) {
    tracing::info!(
        %job_id,
        source_type = source_type_slug(source_type),
        phase = phase.as_str(),
        "job {}",
        phase.as_str()
    );
}

pub fn job_error(job_id: Uuid, source_type: SourceType, err: &(dyn std::error::Error + '_)) {
    let error = sanitize_for_log(&err.to_string());
    tracing::error!(
        %job_id,
        source_type = source_type_slug(source_type),
        phase = "failed",
        %error,
        "job failed"
    );
}

pub fn job_error_message(
    job_id: Uuid,
    source_type: SourceType,
    message: &str,
    failure_kind: Option<&str>,
) {
    let error = sanitize_for_log(message);
    match failure_kind {
        Some(kind) => tracing::error!(
            %job_id,
            source_type = source_type_slug(source_type),
            phase = "failed",
            %error,
            failure_kind = kind,
            "job failed"
        ),
        None => tracing::error!(
            %job_id,
            source_type = source_type_slug(source_type),
            phase = "failed",
            %error,
            "job failed"
        ),
    }
}
