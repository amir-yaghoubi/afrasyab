use crate::error::DomainError;
use crate::types::JobStatus;

pub fn status_name(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Downloading => "downloading",
        JobStatus::Uploading => "uploading",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
    }
}

pub fn transition(from: JobStatus, to: JobStatus) -> Result<JobStatus, DomainError> {
    let allowed = matches!(
        (from, to),
        (JobStatus::Queued, JobStatus::Downloading)
            | (JobStatus::Downloading, JobStatus::Uploading)
            | (JobStatus::Uploading, JobStatus::Completed)
            | (JobStatus::Queued, JobStatus::Failed)
            | (JobStatus::Downloading, JobStatus::Failed)
            | (JobStatus::Uploading, JobStatus::Failed)
    );

    if allowed {
        Ok(to)
    } else {
        Err(DomainError::InvalidTransition(
            status_name(from),
            status_name(to),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let mut s = JobStatus::Queued;
        s = transition(s, JobStatus::Downloading).unwrap();
        s = transition(s, JobStatus::Uploading).unwrap();
        s = transition(s, JobStatus::Completed).unwrap();
        assert_eq!(s, JobStatus::Completed);
    }

    #[test]
    fn cannot_skip_to_completed() {
        let err = transition(JobStatus::Queued, JobStatus::Completed).unwrap_err();
        let DomainError::InvalidTransition(from, to) = err else {
            panic!("expected InvalidTransition");
        };
        assert_eq!(from, "queued");
        assert_eq!(to, "completed");
    }
}
