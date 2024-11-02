use log::{debug, info};
use serde_json::json;
use tokio::sync::RwLock;

use crate::{db::{chat::{chats::ChatsDB, messages::MessagesDB}, internal::error::PPError, user::UsersDB}, server::{
        message::{
            handlers::json_handler::JsonHandler, methods::auth_macros, types::{chat::ChatId, request::send::{MessageId, SendMessageRequest}, response::{events::{NewChatEvent, NewMessageEvent}, send::SendMessageResponse}}
        },
        session::Session,
    }};
use std::sync::Arc;

/// Returns latest chat message id if successful
async fn handle_send_message(
    session: Arc<RwLock<Session>>,
    msg: SendMessageRequest,
    handler: &JsonHandler,
) -> Result<(MessageId, ChatId), PPError> {
    let session = session.read().await;
    let (self_user_id, _) = session.get_credentials_unchecked();
    drop(session);

    // TODO: sending messages on yourself is "Saved Messages"
    if self_user_id.as_i32().unwrap() == msg.common.to {
        return Err(PPError::from("You cannot send messages on yourself!"));
    }

    let users_db: UsersDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();

    // Is Positive? Retrieve real Chat id by the given User Id
    let maybe_chat = if msg.common.to.is_positive() {
        users_db.get_associated_chat_id(&self_user_id, msg.common.to).await?
    } else {if handler.get_db::<ChatsDB>().chat_exists(msg.common.to).await?{
        Some(msg.common.to)
    } else {return Err("No group found by the given chat id!".into())}};

    let associated_chat = match maybe_chat {
        Some(existing_chat_id) => {
            chats_db.fetch_chat(existing_chat_id).await?.ok_or(PPError::from("Failed to find Chat!"))
        }, 
        // Create chat id if doesn't exist
        None => {
            debug!("Message was sent to: {}. Chat with this user wasn't found. Creating chat.", msg.common.to);

            if !users_db.exists(&msg.common.to.into()).await? {
                return Err(PPError::from("Target user_id doesn't exist!"));
            }

            let chat = handler.get_db::<ChatsDB>()
                .create_private(vec![self_user_id.clone(), msg.common.to.into()])
                .await?;
            users_db.add_chat(&self_user_id, msg.common.to, chat.chat_id()).await.unwrap();
            users_db.add_chat(&msg.common.to.into(), self_user_id.as_i32_unchecked(), chat.chat_id()).await.unwrap();

            let mut chat_details = chat.details(&msg.common.to.into()).await?.unwrap();
            chat_details.chat_id = self_user_id.as_i32().unwrap();
            handler.send_msg_to_connection_detached(msg.common.to, NewChatEvent {
                event: "new_chat".into(),
                new_chat: chat_details
            });

            Ok(chat)
        }
    }?;
    
    let messages_db: MessagesDB = handler.get_db();
    let mut db_message = messages_db.add_message(&msg, &self_user_id, associated_chat.chat_id()).await?;
    if !associated_chat.is_group() {
        db_message.chat_id = msg.common.to;
    }
    let message_id = db_message.message_id;

    // Send every user in chat notification
    for participant in associated_chat.participants() {
        // Don't send it on yourself
        if participant.user_id() == self_user_id.as_i32_unchecked() {continue;} 

        handler.send_msg_to_connection_detached(msg.common.to, NewMessageEvent {
            event: "new_message".into(),
            new_message: db_message.clone()
        });
    }
    

    Ok((message_id, msg.common.to))
}

pub async fn handle(handler: &mut JsonHandler, method: &str) {
    auth_macros::require_auth!(handler, method);

    let content = handler.utf8_content_unchecked();
    match serde_json::from_str::<SendMessageRequest>(&content) {
        Ok(msg) => match msg.common.method.as_str() {
            "send_message" => {
                match handle_send_message(Arc::clone(&handler.session), msg, &handler).await {
                    Ok((latest_msg_id, target_chat_id)) => {
                        let data = SendMessageResponse {
                            ok: true,
                            method: "send_message".into(),
                            message_id: latest_msg_id,
                            chat_id: target_chat_id 
                        };
                        handler.send_message(&data).await;
                    }
                    Err(err) => {handler.send_error(method, err).await;},
                }
            }
            _ => handler.send_error(method, "Unknown method given!".into()).await,
        },
        Err(err) => {
            handler.send_error(method, err.to_string().into()).await;
        }
    }
}
