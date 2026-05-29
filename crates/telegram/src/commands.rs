use anyhow::Context;
use teloxide::payloads::SetMyCommandsSetters;
use teloxide::prelude::*;
use teloxide::types::{BotCommandScope, ChatId, Recipient};
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone, Copy, Debug, PartialEq, Eq)]
#[command(rename_rule = "lowercase")]
pub enum UserCommand {
    /// Onboarding and setup
    Start,
    /// Link Google Drive
    Connect,
    /// Set or create upload folder
    Folder,
    /// Recent download jobs
    Status,
    /// Show command list
    Help,
}

#[derive(BotCommands, Clone, Debug, PartialEq, Eq)]
#[command(rename_rule = "lowercase")]
pub enum AdminCommand {
    /// Allowlist a Telegram user ID
    Adduser(i64),
    /// Remove user from allowlist
    Removeuser(i64),
    /// List allowlisted user IDs
    Listusers,
}

fn is_bare_command(text: &str, command: &str, bot_name: &str) -> bool {
    let text = text.trim();
    if text == command {
        return true;
    }
    if !bot_name.is_empty() {
        let with_bot = format!("{command}@{bot_name}");
        if text.eq_ignore_ascii_case(&with_bot) {
            return true;
        }
    }
    false
}

fn user_command_head(cmd: UserCommand) -> &'static str {
    match cmd {
        UserCommand::Start => "/start",
        UserCommand::Connect => "/connect",
        UserCommand::Folder => "/folder",
        UserCommand::Status => "/status",
        UserCommand::Help => "/help",
    }
}

/// Parses a user command only when the message is exactly `/cmd` or `/cmd@bot` (no extra args).
pub fn parse_user_command(text: &str, bot_name: &str) -> Option<UserCommand> {
    let cmd = UserCommand::parse(text, bot_name).ok()?;
    if is_bare_command(text, user_command_head(cmd), bot_name) {
        Some(cmd)
    } else {
        None
    }
}

/// Parses an admin command (`/adduser` and `/removeuser` may include a numeric id).
pub fn parse_admin_command(text: &str, bot_name: &str) -> Option<AdminCommand> {
    AdminCommand::parse(text, bot_name).ok()
}

pub fn help_text() -> String {
    format!(
        "{}\n\nSend a link or file to download to your Drive.",
        UserCommand::descriptions()
    )
}

pub async fn register_bot_commands(
    bot: &Bot,
    super_admin_telegram_id: i64,
) -> anyhow::Result<()> {
    bot.set_my_commands(UserCommand::bot_commands())
        .scope(BotCommandScope::AllPrivateChats)
        .await
        .context("set user commands")?;

    // Private chat id equals user id for DMs with the super-admin.
    bot.set_my_commands(AdminCommand::bot_commands())
        .scope(BotCommandScope::Chat {
            chat_id: Recipient::Id(ChatId(super_admin_telegram_id)),
        })
        .await
        .context("set admin commands")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_start() {
        assert_eq!(
            UserCommand::parse("/start", "").unwrap(),
            UserCommand::Start
        );
    }

    #[test]
    fn parse_user_help_with_bot_suffix() {
        assert_eq!(
            UserCommand::parse("/help@AfrasyabBot", "AfrasyabBot").unwrap(),
            UserCommand::Help
        );
    }

    #[test]
    fn parse_user_status_with_bot_suffix() {
        assert_eq!(
            UserCommand::parse("/status@MyBot", "MyBot").unwrap(),
            UserCommand::Status
        );
    }

    #[test]
    fn reject_user_start_with_extra_args() {
        assert!(parse_user_command("/start foo", "").is_none());
    }

    #[test]
    fn reject_user_folder_with_name() {
        assert!(parse_user_command("/folder My Videos", "").is_none());
    }

    #[test]
    fn parse_admin_adduser() {
        assert_eq!(
            AdminCommand::parse("/adduser 12345", "").unwrap(),
            AdminCommand::Adduser(12345)
        );
    }

    #[test]
    fn parse_admin_listusers() {
        assert_eq!(
            AdminCommand::parse("/listusers", "").unwrap(),
            AdminCommand::Listusers
        );
    }

    #[test]
    fn reject_admin_adduser_without_id() {
        assert!(AdminCommand::parse("/adduser", "").is_err());
    }

    #[test]
    fn help_text_includes_all_user_commands() {
        let text = help_text();
        for cmd in ["start", "connect", "folder", "status", "help"] {
            assert!(text.contains(cmd), "missing {cmd} in help");
        }
        assert!(text.contains("Send a link or file"));
    }
}
