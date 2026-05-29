use afrasyab_domain::types::JobStatus;
use afrasyab_storage::jobs::JobRow;
use uuid::Uuid;

pub fn status_text(job: &JobRow) -> String {
    let status = job.status().unwrap_or(JobStatus::Queued);
    match status {
        JobStatus::Queued => format!("Queued (#{})", short_id(&job.id)),
        JobStatus::Downloading => format!(
            "Downloading (#{}) {}/{}",
            short_id(&job.id),
            job.progress_current,
            job.progress_total
        ),
        JobStatus::Uploading => format!("Uploading to Drive (#{})", short_id(&job.id)),
        JobStatus::Completed => format!("Done ✓ (#{})", short_id(&job.id)),
        JobStatus::Failed => format!(
            "Failed (#{}): {}",
            short_id(&job.id),
            job.error_message.as_deref().unwrap_or("unknown")
        ),
    }
}

pub fn short_id(id: &Uuid) -> String {
    id.to_string()[..8].to_string()
}
