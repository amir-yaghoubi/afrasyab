use afrasyab_storage::drive_settings::DriveSettingsRepo;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::DbPool;
use uuid::Uuid;

/// Returns a user-facing denial message when the user cannot enqueue jobs yet.
pub async fn job_readiness_message(pool: &DbPool, user_id: Uuid) -> Result<(), &'static str> {
    let creds = GoogleCredentialsRepo::new(pool)
        .get_decrypted_refresh(user_id)
        .await
        .map_err(|_| "Could not verify your Google connection. Try again or use /connect.")?;

    if creds.is_none() {
        return Err("Connect Google Drive first with /connect.");
    }

    let folder = DriveSettingsRepo::new(pool)
        .get_folder(user_id)
        .await
        .map_err(|_| "Could not verify your Drive folder. Try /folder.")?;

    if folder.is_none() {
        return Err("Set up your upload folder with /folder before sending files.");
    }

    Ok(())
}
