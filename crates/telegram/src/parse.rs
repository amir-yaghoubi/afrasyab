use serde::{Deserialize, Serialize};
use teloxide::types::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramFileMeta {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IncomingContent {
    pub urls: Vec<url::Url>,
    pub telegram_file: Option<TelegramFileMeta>,
}

pub fn parse_message(msg: &Message) -> IncomingContent {
    let urls = msg
        .text()
        .or_else(|| msg.caption())
        .map(afrasyab_domain::classify::extract_urls)
        .unwrap_or_default();
    let telegram_file = extract_telegram_file(msg);
    IncomingContent {
        urls,
        telegram_file,
    }
}

fn extract_telegram_file(msg: &Message) -> Option<TelegramFileMeta> {
    if let Some(doc) = msg.document() {
        return Some(TelegramFileMeta {
            file_id: doc.file.id.to_string(),
            file_unique_id: doc.file.unique_id.to_string(),
            file_name: doc.file_name.clone(),
            file_size: Some(doc.file.size as u64),
        });
    }
    if let Some(video) = msg.video() {
        return Some(TelegramFileMeta {
            file_id: video.file.id.to_string(),
            file_unique_id: video.file.unique_id.to_string(),
            file_name: video.file_name.clone(),
            file_size: Some(video.file.size as u64),
        });
    }
    if let Some(audio) = msg.audio() {
        return Some(TelegramFileMeta {
            file_id: audio.file.id.to_string(),
            file_unique_id: audio.file.unique_id.to_string(),
            file_name: audio.file_name.clone(),
            file_size: Some(audio.file.size as u64),
        });
    }
    if let Some(voice) = msg.voice() {
        return Some(TelegramFileMeta {
            file_id: voice.file.id.to_string(),
            file_unique_id: voice.file.unique_id.to_string(),
            file_name: None,
            file_size: Some(voice.file.size as u64),
        });
    }
    if let Some(photos) = msg.photo() {
        let largest = photos.last()?;
        return Some(TelegramFileMeta {
            file_id: largest.file.id.to_string(),
            file_unique_id: largest.file.unique_id.to_string(),
            file_name: None,
            file_size: Some(largest.file.size as u64),
        });
    }
    None
}

pub fn is_private_dm(msg: &Message) -> bool {
    matches!(msg.chat.kind, teloxide::types::ChatKind::Private(_))
}

#[cfg(test)]
mod tests {
    use afrasyab_domain::classify::extract_urls;

    #[test]
    fn extracts_urls_from_message_text() {
        let urls = extract_urls("see https://example.com/file.pdf here");
        assert_eq!(urls.len(), 1);
    }
}
