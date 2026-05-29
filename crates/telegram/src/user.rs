use crate::commands::{help_text, UserCommand};
use crate::guards::require_allowed;
use crate::keyboard::folder_action_keyboard;
use crate::keyboard::oauth_url_button;
use crate::onboarding::{onboarding_state, OnboardingState};
use afrasyab_core::AppState;
use afrasyab_drive::DriveUploader;
use afrasyab_storage::drive_settings::DriveSettingsRepo;
use afrasyab_storage::folder_pending::FolderPendingMode;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::jobs::JobsRepo;
use afrasyab_storage::users::UsersRepo;
use std::sync::Arc;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::Message;
use uuid::Uuid;

pub async fn handle_user_command(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    cmd: UserCommand,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id;
    let from = match msg.from.as_ref() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };

    if matches!(cmd, UserCommand::Help) {
        bot.send_message(chat_id, help_text()).await?;
        return Ok(());
    }

    if let Err(denial) = require_allowed(&state.pool, from).await {
        if matches!(cmd, UserCommand::Start) {
            bot.send_message(chat_id, denial).await?;
        }
        return Ok(());
    }

    match cmd {
        UserCommand::Start => handle_start(bot, msg, state, from).await?,
        UserCommand::Connect => handle_connect(bot, chat_id, state, from).await?,
        UserCommand::Folder => handle_folder(bot, chat_id, state, from).await?,
        UserCommand::Status => handle_status(bot, chat_id, state, from).await?,
        UserCommand::Help => {}
    }

    Ok(())
}

async fn handle_start(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    telegram_user_id: i64,
) -> anyhow::Result<()> {
    let chat_id = msg.chat.id;
    let users = UsersRepo::new(&state.pool);
    let user = users.get_or_create_by_telegram_id(telegram_user_id).await?;

    match onboarding_state(&state.pool, &user).await? {
        OnboardingState::NeedsGoogle => {
            handle_connect(bot, chat_id, state, telegram_user_id).await?;
        }
        OnboardingState::NeedsFolder => {
            bot.send_message(
                chat_id,
                "Google Drive is connected. Run /folder to set up your upload folder.",
            )
            .await?;
        }
        OnboardingState::Ready => {
            let folder = DriveSettingsRepo::new(&state.pool)
                .get_folder(user.id)
                .await?;
            let folder_name = folder
                .and_then(|f| f.folder_name)
                .unwrap_or_else(|| "your folder".into());
            bot.send_message(
                chat_id,
                format!(
                    "Afrasyab is ready.\nUpload folder: {folder_name}\n\nSend links or files to download. {}",
                    help_text()
                ),
            )
            .await?;
        }
        OnboardingState::NotAllowed => {
            bot.send_message(chat_id, "You don't have access to Afrasyab. Ask the admin.")
                .await?;
        }
    }
    Ok(())
}

async fn handle_connect(
    bot: Bot,
    chat_id: ChatId,
    state: Arc<AppState>,
    telegram_user_id: i64,
) -> anyhow::Result<()> {
    let oauth_state = Uuid::new_v4().to_string();
    state
        .oauth_store
        .put(&oauth_state, telegram_user_id)
        .await?;

    let url = format!(
        "{}/oauth/google?state={}",
        state.config.public_base_url, oauth_state
    );
    bot.send_message(
        chat_id,
        "Open the button below to connect Google Drive in your browser.",
    )
    .reply_markup(oauth_url_button(&url))
    .await?;
    Ok(())
}

async fn handle_folder(
    bot: Bot,
    chat_id: ChatId,
    state: Arc<AppState>,
    telegram_user_id: i64,
) -> anyhow::Result<()> {
    let user = UsersRepo::new(&state.pool)
        .get_or_create_by_telegram_id(telegram_user_id)
        .await?;

    if GoogleCredentialsRepo::new(&state.pool)
        .get_decrypted_refresh(user.id)
        .await?
        .is_none()
    {
        bot.send_message(chat_id, "Connect Google first with /connect.")
            .await?;
        return Ok(());
    }

    let has_folder = DriveSettingsRepo::new(&state.pool)
        .get_folder(user.id)
        .await?
        .is_some();

    if has_folder {
        bot.send_message(chat_id, "Choose what to do with your upload folder:")
            .reply_markup(folder_action_keyboard())
            .await?;
    } else {
        state
            .folder_pending
            .set_with_mode(telegram_user_id, FolderPendingMode::CreateNew)
            .await?;
        bot.send_message(
            chat_id,
            "Send a name for your upload folder (1–100 characters).",
        )
        .await?;
    }
    Ok(())
}

fn parse_folder_name(input: &str) -> Result<String, &'static str> {
    let name = input.trim();
    if name.is_empty() {
        return Err("Name cannot be empty.");
    }
    if name.starts_with('/') {
        return Err("That looks like a command, not a folder name.");
    }
    if name.len() > 100 {
        return Err("Name must be at most 100 characters.");
    }
    Ok(name.to_string())
}

async fn handle_status(
    bot: Bot,
    chat_id: ChatId,
    state: Arc<AppState>,
    telegram_user_id: i64,
) -> anyhow::Result<()> {
    let user = UsersRepo::new(&state.pool)
        .get_or_create_by_telegram_id(telegram_user_id)
        .await?;

    let jobs = JobsRepo::new(&state.pool)
        .list_recent_for_user(user.id, 10)
        .await?;

    if jobs.is_empty() {
        bot.send_message(chat_id, "No recent jobs.").await?;
        return Ok(());
    }

    let lines: Vec<String> = jobs
        .iter()
        .filter_map(|j| {
            let status = j.status()?;
            Some(format!(
                "#{} — {:?} ({}/{})",
                &j.id.to_string()[..8],
                status,
                j.progress_current,
                j.progress_total
            ))
        })
        .collect();

    bot.send_message(chat_id, lines.join("\n")).await?;
    Ok(())
}

pub async fn try_set_folder_from_message(
    bot: Bot,
    msg: &Message,
    state: Arc<AppState>,
    telegram_user_id: i64,
) -> anyhow::Result<bool> {
    if !state.folder_pending.is_pending(telegram_user_id).await? {
        return Ok(false);
    }

    let mode = match state.folder_pending.get_mode(telegram_user_id).await? {
        Some(mode) => mode,
        None => return Ok(false),
    };

    let name = match parse_folder_name(msg.text().unwrap_or_default()) {
        Ok(name) => name,
        Err(message) => {
            bot.send_message(msg.chat.id, message).await?;
            return Ok(true);
        }
    };

    let users = UsersRepo::new(&state.pool);
    let user = users.get_or_create_by_telegram_id(telegram_user_id).await?;

    let encrypted = GoogleCredentialsRepo::new(&state.pool)
        .get_decrypted_refresh(user.id)
        .await?;
    let Some(encrypted) = encrypted else {
        bot.send_message(msg.chat.id, "Connect Google first with /connect.")
            .await?;
        let _ = state.folder_pending.clear(telegram_user_id).await;
        return Ok(true);
    };

    let refresh_bytes = state
        .cipher
        .decrypt(&encrypted)
        .map_err(|_| anyhow::anyhow!("stored Google credentials are invalid; use /connect"))?;
    let refresh_token = String::from_utf8(refresh_bytes)?;

    let access = state.drive.refresh_access_token(&refresh_token).await?;
    let settings = DriveSettingsRepo::new(&state.pool)
        .get_folder(user.id)
        .await?;

    let confirmation = match mode {
        FolderPendingMode::Rename => {
            let Some(row) = settings else {
                bot.send_message(msg.chat.id, "No folder to rename. Run /folder again.")
                    .await?;
                state.folder_pending.clear(telegram_user_id).await?;
                return Ok(true);
            };
            let display = state
                .drive
                .rename_folder(&access, &row.folder_id, &name)
                .await?;
            DriveSettingsRepo::new(&state.pool)
                .upsert_folder(user.id, &row.folder_id, Some(&display))
                .await?;
            format!("Folder renamed to “{display}”.")
        }
        FolderPendingMode::CreateNew => {
            let (folder_id, display) = state.drive.create_folder(&access, &name, None).await?;
            DriveSettingsRepo::new(&state.pool)
                .upsert_folder(user.id, &folder_id, Some(&display))
                .await?;
            format!("Uploads now go to “{display}”.")
        }
    };

    state.folder_pending.clear(telegram_user_id).await?;
    users.set_onboarding_complete(user.id).await?;

    bot.send_message(msg.chat.id, confirmation).await?;
    Ok(true)
}
