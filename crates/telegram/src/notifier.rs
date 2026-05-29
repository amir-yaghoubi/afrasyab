use async_trait::async_trait;
use teloxide::payloads::{EditMessageTextSetters, SendMessageSetters};
use teloxide::requests::Requester;
use teloxide::types::{ChatId, MessageId, ParseMode};
use teloxide::Bot;

#[async_trait]
pub trait TelegramNotifier: Send + Sync {
    async fn send_message(&self, chat_id: i64, text: &str) -> anyhow::Result<i64>;

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> anyhow::Result<()>;
}

pub struct TeloxideNotifier {
    bot: Bot,
}

impl TeloxideNotifier {
    pub fn new(bot: Bot) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl TelegramNotifier for TeloxideNotifier {
    async fn send_message(&self, chat_id: i64, text: &str) -> anyhow::Result<i64> {
        let msg = self
            .bot
            .send_message(ChatId(chat_id), text)
            .parse_mode(ParseMode::Html)
            .await?;
        Ok(msg.id.0 as i64)
    }

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> anyhow::Result<()> {
        self.bot
            .edit_message_text(ChatId(chat_id), MessageId(message_id as i32), text)
            .parse_mode(ParseMode::Html)
            .await?;
        Ok(())
    }
}
