use crate::DbPool;
use afrasyab_domain::job::status_name;
use afrasyab_domain::types::{JobStatus, SourceType};
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobRow {
    #[sqlx(try_from = "String")]
    pub id: Uuid,
    #[sqlx(try_from = "String")]
    pub user_id: Uuid,
    pub status: String,
    pub source_type: String,
    pub source_meta: Value,
    pub telegram_chat_id: i64,
    pub status_message_id: Option<i64>,
    pub progress_current: i32,
    pub progress_total: i32,
    pub error_message: Option<String>,
    pub drive_file_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobRow {
    pub fn status(&self) -> Option<JobStatus> {
        parse_job_status(&self.status)
    }

    pub fn source_type(&self) -> Option<SourceType> {
        parse_source_type(&self.source_type)
    }
}

pub struct NewJob<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub source_type: SourceType,
    pub source_meta: &'a Value,
    pub telegram_chat_id: i64,
}

pub struct JobsRepo<'a> {
    pool: &'a DbPool,
}

impl<'a> JobsRepo<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, job: NewJob<'_>) -> sqlx::Result<JobRow> {
        sqlx::query_as(
            "INSERT INTO jobs (
                id, user_id, source_type, source_meta, telegram_chat_id
             ) VALUES (?, ?, ?, ?, ?)
             RETURNING id, user_id, status, source_type, source_meta,
                       telegram_chat_id, status_message_id, progress_current,
                       progress_total, error_message, drive_file_id,
                       created_at, updated_at",
        )
        .bind(job.id.to_string())
        .bind(job.user_id.to_string())
        .bind(source_type_name(job.source_type))
        .bind(job.source_meta)
        .bind(job.telegram_chat_id)
        .fetch_one(self.pool)
        .await
    }

    pub async fn claim_next_queued(&self) -> sqlx::Result<Option<Uuid>> {
        let row: Option<(String,)> = sqlx::query_as(
            "UPDATE jobs
             SET status = 'downloading', updated_at = datetime('now')
             WHERE id = (
               SELECT id FROM jobs
               WHERE status = 'queued'
               ORDER BY created_at ASC
               LIMIT 1
             )
             RETURNING id",
        )
        .fetch_optional(self.pool)
        .await?;
        row.map(|(id,)| Uuid::parse_str(&id))
            .transpose()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }

    pub async fn update_status(
        &self,
        job_id: Uuid,
        status: JobStatus,
        error_message: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE jobs
             SET status = ?,
                 error_message = ?,
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(status_name(status))
        .bind(error_message)
        .bind(job_id.to_string())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_progress(
        &self,
        job_id: Uuid,
        progress_current: i32,
        progress_total: i32,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE jobs
             SET progress_current = ?,
                 progress_total = ?,
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(progress_current)
        .bind(progress_total)
        .bind(job_id.to_string())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_status_message_id(
        &self,
        job_id: Uuid,
        status_message_id: i64,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE jobs
             SET status_message_id = ?,
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(status_message_id)
        .bind(job_id.to_string())
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_id(&self, job_id: Uuid) -> sqlx::Result<Option<JobRow>> {
        sqlx::query_as(
            "SELECT id, user_id, status, source_type, source_meta,
                    telegram_chat_id, status_message_id, progress_current,
                    progress_total, error_message, drive_file_id,
                    created_at, updated_at
             FROM jobs WHERE id = ?",
        )
        .bind(job_id.to_string())
        .fetch_optional(self.pool)
        .await
    }

    pub async fn set_drive_file_id(&self, job_id: Uuid, drive_file_id: &str) -> sqlx::Result<()> {
        sqlx::query("UPDATE jobs SET drive_file_id = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(drive_file_id)
            .bind(job_id.to_string())
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_recent_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> sqlx::Result<Vec<JobRow>> {
        sqlx::query_as(
            "SELECT id, user_id, status, source_type, source_meta,
                    telegram_chat_id, status_message_id, progress_current,
                    progress_total, error_message, drive_file_id,
                    created_at, updated_at
             FROM jobs
             WHERE user_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(user_id.to_string())
        .bind(limit)
        .fetch_all(self.pool)
        .await
    }
}

fn source_type_name(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::LinkYtDlp => "link_yt_dlp",
        SourceType::LinkDirect => "link_direct",
        SourceType::TelegramFile => "telegram_file",
        SourceType::PlaylistItem => "playlist_item",
    }
}

fn parse_job_status(raw: &str) -> Option<JobStatus> {
    match raw {
        "queued" => Some(JobStatus::Queued),
        "downloading" => Some(JobStatus::Downloading),
        "uploading" => Some(JobStatus::Uploading),
        "completed" => Some(JobStatus::Completed),
        "failed" => Some(JobStatus::Failed),
        _ => None,
    }
}

fn parse_source_type(raw: &str) -> Option<SourceType> {
    match raw {
        "link_yt_dlp" => Some(SourceType::LinkYtDlp),
        "link_direct" => Some(SourceType::LinkDirect),
        "telegram_file" => Some(SourceType::TelegramFile),
        "playlist_item" => Some(SourceType::PlaylistItem),
        _ => None,
    }
}
