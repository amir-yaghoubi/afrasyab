use crate::config::Config;
use crate::scratch_disk::DiskPressureGate;
use afrasyab_domain::crypto::TokenCipher;
use afrasyab_drive::GoogleDriveClient;
use afrasyab_storage::folder_pending::FolderPendingStore;
use afrasyab_storage::oauth_states::OAuthStateStore;
use afrasyab_storage::pending_format::PendingFormatStore;
use afrasyab_storage::pending_playlist::PendingPlaylistStore;
use afrasyab_storage::DbPool;
use std::sync::Arc;

const OAUTH_STATE_TTL_SECS: u64 = 600;
const PENDING_TTL_SECS: u64 = 600;
const FOLDER_PENDING_TTL_SECS: u64 = 300;

pub struct AppState {
    pub config: Arc<Config>,
    pub pool: DbPool,
    pub oauth_store: OAuthStateStore,
    pub cipher: TokenCipher,
    pub pending_format: PendingFormatStore,
    pub pending_playlist: PendingPlaylistStore,
    pub folder_pending: FolderPendingStore,
    pub drive: Arc<GoogleDriveClient>,
    pub disk_pressure: DiskPressureGate,
}

impl AppState {
    pub async fn new(config: Config, pool: DbPool) -> anyhow::Result<Arc<Self>> {
        let config = Arc::new(config);
        let pool_arc = Arc::new(pool.clone());
        Ok(Arc::new(Self {
            oauth_store: OAuthStateStore::new(pool_arc.clone(), OAUTH_STATE_TTL_SECS),
            cipher: TokenCipher::new(&config.token_encryption_key),
            pending_format: PendingFormatStore::new(pool_arc.clone(), PENDING_TTL_SECS),
            pending_playlist: PendingPlaylistStore::new(pool_arc.clone(), PENDING_TTL_SECS),
            folder_pending: FolderPendingStore::new(pool_arc, FOLDER_PENDING_TTL_SECS),
            drive: Arc::new(
                GoogleDriveClient::new(
                    config.google_client_id.clone(),
                    config.google_client_secret.clone(),
                )
                .with_file_limits(config.max_file_bytes, config.drive_upload_chunk_bytes)?,
            ),
            config,
            pool,
            disk_pressure: DiskPressureGate::new(),
        }))
    }
}
