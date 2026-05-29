use crate::job_runner::JobRunner;
use afrasyab_core::job_log::{job_event, JobPhase};
use afrasyab_core::scratch_disk::free_bytes_at;
use afrasyab_core::AppState;
use afrasyab_downloader::CompositeDownloader;
use afrasyab_drive::GoogleDriveClient;
use afrasyab_storage::jobs::JobsRepo;
use afrasyab_telegram::{TelegramNotifier, TeloxideNotifier};
use std::sync::Arc;
use std::time::Duration;
use teloxide::Bot;

const IDLE_POLL_MS: u64 = 500;
const DISK_PRESSURE_POLL_MS: u64 = 15_000;

pub async fn run_pool(state: Arc<AppState>) {
    let bot = Bot::new(state.config.telegram_bot_token.clone());
    let notifier: Arc<dyn TelegramNotifier> = Arc::new(TeloxideNotifier::new(bot));
    let downloader = Arc::new(CompositeDownloader::new(
        &state.config.telegram_bot_token,
        state.config.max_file_bytes,
    ));
    let drive: Arc<GoogleDriveClient> = state.drive.clone();

    let runner = Arc::new(JobRunner {
        downloader,
        drive,
        notifier,
        state: state.clone(),
    });

    let workers = state.config.max_concurrent_jobs;
    tracing::info!(workers, "starting job worker pool");

    for worker_id in 0..workers {
        let runner = runner.clone();
        let state = state.clone();
        tokio::spawn(async move {
            tracing::debug!(worker_id, "worker started");
            loop {
                let scratch = std::env::temp_dir();
                let free = free_bytes_at(&scratch).unwrap_or(0);
                let thresholds = &state.config.scratch_thresholds;
                if state.disk_pressure.evaluate(free, thresholds) {
                    tokio::time::sleep(Duration::from_millis(DISK_PRESSURE_POLL_MS)).await;
                    continue;
                }

                let jobs = JobsRepo::new(&state.pool);
                match jobs.claim_next_queued().await {
                    Ok(Some(job_id)) => {
                        if let Ok(Some(row)) = jobs.get_by_id(job_id).await {
                            if let Some(source_type) = row.source_type() {
                                job_event(job_id, source_type, JobPhase::Claimed);
                            }
                        }
                        if let Err(e) = runner.run_job(job_id).await {
                            tracing::debug!(%job_id, error = %e, "run_job returned error");
                        }
                    }
                    Ok(None) => {
                        tokio::time::sleep(Duration::from_millis(IDLE_POLL_MS)).await;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "claim job failed");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }
}
