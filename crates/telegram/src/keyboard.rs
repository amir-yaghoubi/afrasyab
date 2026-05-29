use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

pub fn format_keyboard(pending_id: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("Video", format!("fmt:video:{pending_id}")),
        InlineKeyboardButton::callback("Audio", format!("fmt:audio:{pending_id}")),
        InlineKeyboardButton::callback("Best", format!("fmt:best:{pending_id}")),
    ]])
}

pub fn playlist_confirm_keyboard(pending_id: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("Yes", format!("pl:yes:{pending_id}")),
        InlineKeyboardButton::callback("No", format!("pl:no:{pending_id}")),
    ]])
}

pub fn folder_action_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("Rename current folder", "folder:rename"),
        InlineKeyboardButton::callback("Create new folder", "folder:new"),
    ]])
}

pub fn oauth_url_button(url: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::url(
        "Connect Google Drive",
        url.parse().expect("valid oauth url"),
    )]])
}
