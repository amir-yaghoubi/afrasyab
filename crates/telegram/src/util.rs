use teloxide::prelude::*;
use teloxide::types::MaybeInaccessibleMessage;

pub async fn edit_callback_message(
    bot: &Bot,
    msg: MaybeInaccessibleMessage,
    text: &str,
) -> ResponseResult<()> {
    if let MaybeInaccessibleMessage::Regular(message) = msg {
        bot.edit_message_text(message.chat.id, message.id, text)
            .await?;
    }
    Ok(())
}
