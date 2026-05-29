DELETE FROM drive_settings;

ALTER TABLE folder_pending ADD COLUMN mode TEXT NOT NULL DEFAULT 'create_new';
