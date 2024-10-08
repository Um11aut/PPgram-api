use log::debug;
use serde_json::json;
use tokio::sync::RwLock;

use crate::{db::{chat::{chats::ChatsDB, messages::MessagesDB}, internal::error::{PPError, PPResult}, user::UsersDB}, server::{
        message::{
            handler::{Handler, SesssionArcRwLock}, types::{chat::{Chat, ChatDetails, ChatId}, request::{extract_what_field, message::{MessageId, MessageRequest}, new::NewGroupRequest}, response::{events::{NewChatEvent, NewMessageEvent}, send::SendMessageResponse}, user::UserId}
        },
        session::Session,
    }};
use std::sync::Arc;

/// Returns latest chat message id if sucessful
async fn handle_new_group(
    msg: NewGroupRequest,
    handler: &Handler,
) -> Result<Chat, PPError> {
    let self_user_id: UserId = {
        handler.session.read().await.get_credentials().unwrap().0.to_owned()
    };
    let group = handler.get_db::<ChatsDB>().create_group(vec![self_user_id], ChatDetails {
        name: msg.name,
        chat_id: Default::default(),
        username: msg.username,
        photo: msg.avatar_hash
    }).await?;

    Ok(group)
}

async fn on_new(handler: &mut Handler) -> PPResult<Chat> {
    let content = handler.utf8_content_unchecked();
    let what_field = extract_what_field(content)?;

    match what_field.as_str() {
        "group" => match serde_json::from_str::<NewGroupRequest>(&content) {
            Ok(msg) => {
                Ok(handle_new_group(msg, handler).await?)
            },
            Err(err) => {
                Err(err.into())
            },
        },
        _ => Err("Unknown what field!".into())
    }
}

pub async fn handle(handler: &mut Handler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler.send_error(method, "You aren't authenticated!".into()).await;
            return;
        }
    }

    match on_new(handler).await {
        Ok(chat) => {
            handler.send_message(&json!({
                "ok": true,
                "method": "new_group",
                "chat_id": chat.chat_id()
            })).await;
        }
        Err(err) => handler.send_error(method, err).await
    }
}
