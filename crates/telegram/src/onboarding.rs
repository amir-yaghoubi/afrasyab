use afrasyab_storage::drive_settings::DriveSettingsRepo;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::users::UserRow;
use afrasyab_storage::DbPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingState {
    NotAllowed,
    NeedsGoogle,
    NeedsFolder,
    Ready,
}

pub async fn onboarding_state(pool: &DbPool, user: &UserRow) -> anyhow::Result<OnboardingState> {
    let creds = GoogleCredentialsRepo::new(pool)
        .get_decrypted_refresh(user.id)
        .await?;
    if creds.is_none() {
        return Ok(OnboardingState::NeedsGoogle);
    }
    let folder = DriveSettingsRepo::new(pool).get_folder(user.id).await?;
    if folder.is_none() {
        return Ok(OnboardingState::NeedsFolder);
    }
    Ok(OnboardingState::Ready)
}

pub fn short_user_id(id: &Uuid) -> String {
    id.to_string()[..8].to_string()
}
