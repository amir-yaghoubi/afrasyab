use crate::admin::handle_admin_command;
use crate::commands::{parse_admin_command, parse_user_command};
use crate::enqueue::{direct_meta, insert_and_enqueue, new_job_id, telegram_file_meta, ytdlp_meta};
use crate::guards::{is_super_admin, require_allowed};
use crate::keyboard::{format_keyboard, playlist_confirm_keyboard};
use crate::notifier::{TelegramNotifier, TeloxideNotifier};
use crate::parse::{is_private_dm, parse_message, IncomingContent};
use crate::playlist::{detect_playlist_size, fetch_playlist_entry_urls};
use crate::readiness::job_readiness_message;
use crate::status::status_text;
use crate::user::{handle_user_command, try_set_folder_from_message};
use crate::util::edit_callback_message;
use afrasyab_core::AppState;
use afrasyab_domain::classify::{classify_url, LinkKind};
use afrasyab_domain::types::{SourceType, YtDlpFormat};
use afrasyab_storage::folder_pending::FolderPendingMode;
use afrasyab_storage::google_credentials::GoogleCredentialsRepo;
use afrasyab_storage::jobs::{JobsRepo, NewJob};
use afrasyab_storage::pending_format::PendingFormat;
use afrasyab_storage::pending_playlist::PendingPlaylist;
use afrasyab_storage::users::UsersRepo;
use std::sync::Arc;
use teloxide::payloads::{AnswerCallbackQuerySetters, SendMessageSetters};
use teloxide::prelude::*;
use teloxide::types::{CallbackQuery, Message};
pub async fn handle_message(bot: Bot, msg: Message, state: Arc<AppState>) -> ResponseResult<()> {
    match handle_message_impl(bot, msg, state).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(error = %e, "handle_message failed");
            Ok(())
        }
    }
}

async fn handle_message_impl(bot: Bot, msg: Message, state: Arc<AppState>) -> anyhow::Result<()> {
    if !is_private_dm(&msg) {
        return Ok(());
    }

    let text = msg.text().unwrap_or_default();

    let bot_name = bot
        .get_me()
        .await?
        .username
        .clone()
        .unwrap_or_default();

    let from = match msg.from.as_ref() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };

    if is_super_admin(from, &state.config) {
        if let Some(cmd) = parse_admin_command(text, &bot_name) {
            return handle_admin_command(bot, msg, state, cmd).await;
        }
        if text.starts_with("/adduser") {
            bot.send_message(msg.chat.id, "Usage: /adduser <telegram_id>")
                .await?;
            return Ok(());
        }
        if text.starts_with("/removeuser") {
            bot.send_message(msg.chat.id, "Usage: /removeuser <telegram_id>")
                .await?;
            return Ok(());
        }
    }

    if let Some(cmd) = parse_user_command(text, &bot_name) {
        return handle_user_command(bot, msg, state, cmd).await;
    }

    if let Err(denial) = require_allowed(&state.pool, from).await {
        bot.send_message(msg.chat.id, denial).await?;
        return Ok(());
    }

    if try_set_folder_from_message(bot.clone(), &msg, state.clone(), from).await? {
        return Ok(());
    }

    let content = parse_message(&msg);
    if content.urls.is_empty() && content.telegram_file.is_none() {
        return Ok(());
    }

    let notifier: Arc<dyn TelegramNotifier> = Arc::new(TeloxideNotifier::new(bot.clone()));
    process_incoming(bot, msg, state, from, content, notifier).await
}

pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    match handle_callback_impl(bot, q, state).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(error = %e, "handle_callback failed");
            Ok(())
        }
    }
}

async fn handle_callback_impl(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    let data = q.data.clone().unwrap_or_default();

    let from = q.from.id.0 as i64;
    if require_allowed(&state.pool, from).await.is_err() {
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    let notifier: Arc<dyn TelegramNotifier> = Arc::new(TeloxideNotifier::new(bot.clone()));

    if let Some(rest) = data.strip_prefix("fmt:") {
        handle_format_callback(bot.clone(), q, state, notifier, rest).await?;
    } else if let Some(rest) = data.strip_prefix("pl:") {
        handle_playlist_callback(bot.clone(), q, state, notifier, rest).await?;
    } else if data == "folder:rename" {
        handle_folder_mode_callback(bot.clone(), q, state, FolderPendingMode::Rename).await?;
    } else if data == "folder:new" {
        handle_folder_mode_callback(bot.clone(), q, state, FolderPendingMode::CreateNew).await?;
    } else {
        bot.answer_callback_query(q.id).await?;
    }

    Ok(())
}

async fn process_incoming(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    telegram_user_id: i64,
    content: IncomingContent,
    notifier: Arc<dyn TelegramNotifier>,
) -> anyhow::Result<()> {
    let user = UsersRepo::new(&state.pool)
        .get_or_create_by_telegram_id(telegram_user_id)
        .await?;

    let chat_id = msg.chat.id.0;

    for url in &content.urls {
        let raw = url.as_str();
        let (kind, _) = match classify_url(raw) {
            Some(k) => k,
            None => continue,
        };

        match kind {
            LinkKind::DirectHttp => {
                let meta = direct_meta(raw);
                insert_and_enqueue(
                    state.clone(),
                    notifier.clone(),
                    NewJob {
                        id: new_job_id(),
                        user_id: user.id,
                        source_type: SourceType::LinkDirect,
                        source_meta: &meta,
                        telegram_chat_id: chat_id,
                    },
                )
                .await?;
            }
            LinkKind::YtDlp => {
                if let Some(count) = detect_playlist_size(raw).await? {
                    if count > state.config.max_playlist_items {
                        bot.send_message(
                            msg.chat.id,
                            format!(
                                "Playlist has {count} items (max {}).",
                                state.config.max_playlist_items
                            ),
                        )
                        .await?;
                        continue;
                    }
                    let entry_urls = fetch_playlist_entry_urls(raw).await?;
                    if entry_urls.len() > 1 {
                        let pending_id = uuid::Uuid::new_v4().to_string();
                        state
                            .pending_playlist
                            .put(
                                &pending_id,
                                &PendingPlaylist {
                                    url: raw.to_string(),
                                    entry_urls,
                                    telegram_user_id,
                                    telegram_chat_id: chat_id,
                                    user_id: user.id,
                                },
                            )
                            .await?;
                        bot.send_message(
                            msg.chat.id,
                            format!("Download all {count} items from this playlist?"),
                        )
                        .reply_markup(playlist_confirm_keyboard(&pending_id))
                        .await?;
                        continue;
                    }
                }

                let pending_id = uuid::Uuid::new_v4().to_string();
                state
                    .pending_format
                    .put(
                        &pending_id,
                        &PendingFormat {
                            url: raw.to_string(),
                            telegram_user_id,
                            telegram_chat_id: chat_id,
                            user_id: user.id,
                        },
                    )
                    .await?;
                bot.send_message(msg.chat.id, "Choose a format:")
                    .reply_markup(format_keyboard(&pending_id))
                    .await?;
            }
        }
    }

    if let Some(file) = content.telegram_file {
        const TELEGRAM_MAX_BYTES: u64 = 20 * 1024 * 1024;
        if file.file_size.is_some_and(|s| s > TELEGRAM_MAX_BYTES) {
            bot.send_message(
                msg.chat.id,
                "File too large for the Telegram bot API (max 20 MB). Send a direct link instead.",
            )
            .await?;
        } else {
            let meta = telegram_file_meta(
                &file.file_id,
                &file.file_unique_id,
                file.file_name.as_deref(),
                file.file_size,
            );
            insert_and_enqueue(
                state,
                notifier,
                NewJob {
                    id: new_job_id(),
                    user_id: user.id,
                    source_type: SourceType::TelegramFile,
                    source_meta: &meta,
                    telegram_chat_id: chat_id,
                },
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_folder_mode_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
    mode: FolderPendingMode,
) -> anyhow::Result<()> {
    let from = q.from.id.0 as i64;
    let chat_id = q
        .message
        .as_ref()
        .map(|m| m.chat().id)
        .unwrap_or(ChatId(from));

    let user = UsersRepo::new(&state.pool)
        .get_or_create_by_telegram_id(from)
        .await?;

    if GoogleCredentialsRepo::new(&state.pool)
        .get_decrypted_refresh(user.id)
        .await?
        .is_none()
    {
        bot.answer_callback_query(q.id)
            .text("Connect Google first with /connect.")
            .await?;
        return Ok(());
    }

    state.folder_pending.set_with_mode(from, mode).await?;
    bot.answer_callback_query(q.id).await?;
    bot.send_message(chat_id, "Send the new folder name (1–100 characters).")
        .await?;
    Ok(())
}

async fn handle_format_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
    notifier: Arc<dyn TelegramNotifier>,
    data: &str,
) -> anyhow::Result<()> {
    let mut parts = data.splitn(3, ':');
    let format_key = parts.next().unwrap_or_default();
    let pending_id = parts.next().unwrap_or_default();

    let format = match format_key {
        "video" => YtDlpFormat::Video,
        "audio" => YtDlpFormat::Audio,
        "best" => YtDlpFormat::Best,
        _ => {
            bot.answer_callback_query(q.id).await?;
            return Ok(());
        }
    };

    let pending = state.pending_format.take(pending_id).await?;

    let Some(pending) = pending else {
        bot.answer_callback_query(q.id)
            .text("This prompt expired. Send the link again.")
            .await?;
        return Ok(());
    };

    let format_str = match format {
        YtDlpFormat::Video => "video",
        YtDlpFormat::Audio => "audio",
        YtDlpFormat::Best => "best",
    };
    let meta = ytdlp_meta(&pending.url, format_str);
    let queued = insert_and_enqueue(
        state,
        notifier,
        NewJob {
            id: new_job_id(),
            user_id: pending.user_id,
            source_type: SourceType::LinkYtDlp,
            source_meta: &meta,
            telegram_chat_id: pending.telegram_chat_id,
        },
    )
    .await?;

    let ack = if queued.is_some() {
        "Queued."
    } else {
        "Not queued — complete /connect and /folder first."
    };
    bot.answer_callback_query(q.id).text(ack).await?;
    if let Some(msg) = q.message {
        let _ = edit_callback_message(&bot, msg, "Queued for download.").await;
    }
    Ok(())
}

async fn handle_playlist_callback(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
    notifier: Arc<dyn TelegramNotifier>,
    data: &str,
) -> anyhow::Result<()> {
    let mut parts = data.splitn(2, ':');
    let answer = parts.next().unwrap_or_default();
    let pending_id = parts.next().unwrap_or_default();

    let pending = state.pending_playlist.take(pending_id).await?;

    let Some(pending) = pending else {
        bot.answer_callback_query(q.id)
            .text("This prompt expired. Send the playlist link again.")
            .await?;
        return Ok(());
    };

    if answer != "yes" {
        bot.answer_callback_query(q.id).await?;
        if let Some(msg) = q.message {
            let _ = edit_callback_message(&bot, msg, "Playlist download cancelled.").await;
        }
        return Ok(());
    }

    if let Err(message) = job_readiness_message(&state.pool, pending.user_id).await {
        bot.answer_callback_query(q.id).text(message).await?;
        if let Some(msg) = q.message {
            let _ = edit_callback_message(&bot, msg, message).await;
        }
        return Ok(());
    }

    let total = pending.entry_urls.len() as i32;
    for (idx, entry_url) in pending.entry_urls.iter().enumerate() {
        let meta = ytdlp_meta(entry_url, "best");
        let job_id = new_job_id();
        let jobs = JobsRepo::new(&state.pool);
        let row = jobs
            .insert(NewJob {
                id: job_id,
                user_id: pending.user_id,
                source_type: SourceType::PlaylistItem,
                source_meta: &meta,
                telegram_chat_id: pending.telegram_chat_id,
            })
            .await?;
        jobs.update_progress(job_id, idx as i32 + 1, total).await?;
        let text = status_text(&row);
        let message_id = notifier
            .send_message(pending.telegram_chat_id, &text)
            .await?;
        jobs.set_status_message_id(job_id, message_id).await?;
    }

    bot.answer_callback_query(q.id)
        .text(format!("Queued {total} items."))
        .await?;
    if let Some(msg) = q.message {
        let _ = edit_callback_message(&bot, msg, &format!("Queued {total} playlist items.")).await;
    }
    Ok(())
}
