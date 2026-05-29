use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type TelegramUserId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Downloading,
    Uploading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    LinkYtDlp,
    LinkDirect,
    TelegramFile,
    PlaylistItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YtDlpFormat {
    Video,
    Audio,
    Best,
}
