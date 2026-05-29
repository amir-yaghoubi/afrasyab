//! SQLx-based SQLite persistence for Afrasyab.

pub mod allowed_users;
pub mod drive_settings;
pub mod ephemeral;
pub mod folder_pending;
pub mod google_credentials;
pub mod jobs;
pub mod oauth_states;
pub mod pending_format;
pub mod pending_playlist;
pub mod pool;
pub mod users;

pub use pool::{connect, DbPool};
