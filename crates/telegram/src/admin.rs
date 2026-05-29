use crate::commands::AdminCommand;
use crate::guards::is_super_admin;
use afrasyab_core::AppState;
use afrasyab_storage::allowed_users::AllowedUsersRepo;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::Message;

pub async fn handle_admin_command(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
    cmd: AdminCommand,
) -> anyhow::Result<()> {
    let from = match msg.from.as_ref() {
        Some(u) => u.id.0 as i64,
        None => return Ok(()),
    };

    if !is_super_admin(from, &state.config) {
        return Ok(());
    }

    let chat_id = msg.chat.id;

    match cmd {
        AdminCommand::Adduser(target) => {
            AllowedUsersRepo::new(&state.pool).add(target, from).await?;
            bot.send_message(chat_id, format!("Added user {target}."))
                .await?;
        }
        AdminCommand::Removeuser(target) => {
            AllowedUsersRepo::new(&state.pool).remove(target).await?;
            bot.send_message(chat_id, format!("Removed user {target}."))
                .await?;
        }
        AdminCommand::Listusers => {
            let ids = AllowedUsersRepo::new(&state.pool).list().await?;
            if ids.is_empty() {
                bot.send_message(chat_id, "No allowlisted users.").await?;
            } else {
                let body = ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                bot.send_message(chat_id, format!("Allowlisted users:\n{body}"))
                    .await?;
            }
        }
    }

    Ok(())
}
