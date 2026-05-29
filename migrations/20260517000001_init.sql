CREATE TABLE allowed_users (
    telegram_user_id INTEGER NOT NULL PRIMARY KEY,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    added_by INTEGER NOT NULL
);

CREATE TABLE users (
    id TEXT NOT NULL PRIMARY KEY,
    telegram_user_id INTEGER NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    onboarding_complete INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE google_credentials (
    user_id TEXT NOT NULL PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    encrypted_refresh_token BLOB NOT NULL,
    access_token TEXT,
    access_token_expires_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE drive_settings (
    user_id TEXT NOT NULL PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    folder_id TEXT NOT NULL,
    folder_name TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE jobs (
    id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','downloading','uploading','completed','failed')),
    source_type TEXT NOT NULL CHECK (source_type IN ('link_yt_dlp','link_direct','telegram_file','playlist_item')),
    source_meta TEXT NOT NULL DEFAULT '{}',
    telegram_chat_id INTEGER NOT NULL,
    status_message_id INTEGER,
    progress_current INTEGER NOT NULL DEFAULT 0,
    progress_total INTEGER NOT NULL DEFAULT 1,
    error_message TEXT,
    drive_file_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX jobs_user_id_created_at_idx ON jobs(user_id, created_at DESC);
CREATE INDEX jobs_status_created_at_idx ON jobs(status, created_at ASC);

CREATE TABLE oauth_states (
    state TEXT NOT NULL PRIMARY KEY,
    telegram_user_id INTEGER NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE pending_formats (
    id TEXT NOT NULL PRIMARY KEY,
    payload TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE pending_playlists (
    id TEXT NOT NULL PRIMARY KEY,
    payload TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE folder_pending (
    telegram_user_id INTEGER NOT NULL PRIMARY KEY,
    expires_at TEXT NOT NULL
);
