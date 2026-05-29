use crate::notifier::TelegramNotifier;
use crate::readiness::job_readiness_message;
use crate::status::status_text;
use afrasyab_core::job_log::{job_event, JobPhase};
use afrasyab_core::AppState;
use afrasyab_storage::jobs::{JobRow, JobsRepo, NewJob};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

/// Enqueues a job when the user has Google credentials and a Drive folder configured.
/// Returns `Ok(None)` after notifying the user if not ready (no job created).
pub async fn insert_and_enqueue(
    state: Arc<AppState>,
    notifier: Arc<dyn TelegramNotifier>,
    new_job: NewJob<'_>,
) -> anyhow::Result<Option<JobRow>> {
    if let Err(message) = job_readiness_message(&state.pool, new_job.user_id).await {
        notifier
            .send_message(new_job.telegram_chat_id, message)
            .await?;
        return Ok(None);
    }

    let jobs = JobsRepo::new(&state.pool);
    let source_type = new_job.source_type;
    let row = jobs.insert(new_job).await?;
    job_event(row.id, source_type, JobPhase::Queued);
    let text = status_text(&row);
    let message_id = notifier.send_message(row.telegram_chat_id, &text).await?;
    jobs.set_status_message_id(row.id, message_id).await?;
    Ok(Some(row))
}

pub fn ytdlp_meta(url: &str, format: &str) -> Value {
    json!({ "url": url, "format": format })
}

pub fn direct_meta(url: &str) -> Value {
    json!({ "url": url })
}

pub fn telegram_file_meta(
    file_id: &str,
    file_unique_id: &str,
    file_name: Option<&str>,
    file_size: Option<u64>,
) -> Value {
    json!({
        "file_id": file_id,
        "file_unique_id": file_unique_id,
        "file_name": file_name,
        "file_size": file_size,
    })
}

pub fn new_job_id() -> Uuid {
    Uuid::new_v4()
}
