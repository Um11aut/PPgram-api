use log::debug;
use serde_json::json;
use tokio::sync::RwLock;

use crate::{db::{chat::{chats::ChatsDB, messages::MessagesDB}, internal::error::PPError, user::UsersDB}, server::{
        message::{
            handlers::tcp_handler::TCPHandler, types::{chat::ChatId, request::message::{MessageId, MessageRequest}, response::{events::{NewChatEvent, NewMessageEvent}, send::SendMessageResponse}}
        },
        session::Session,
    }};
use std::sync::Arc;

/// Returns latest chat message id if sucessful
async fn handle_send_message(
    session: Arc<RwLock<Session>>,
    msg: MessageRequest,
    handler: &TCPHandler,
) -> Result<(MessageId, ChatId), PPError> {
    let session = session.read().await;
    let (self_user_id, _) = session.get_credentials_unchecked();
    drop(session);

    // TODO: sending messages on yourself is "Saved Messages"
    if self_user_id.as_i32().unwrap() == msg.common.to {
        return Err(PPError::from("You cannot send messages on yourself!"));
    }

    let users_db: UsersDB = handler.get_db();

    // Is Positive? Retreive real Chat id by the given User Id
    let maybe_chat = if msg.common.to.is_positive() {
        users_db.get_associated_chat_id(&self_user_id, &msg.common.to.into()).await?
    } else {if handler.get_db::<ChatsDB>().chat_exists(msg.common.to).await?{
        Some(msg.common.to)
    } else {return Err("No group found by the given chat id!".into())}};

    let associated_chat_id = match maybe_chat {
        Some(existing_chat_id) => {
            existing_chat_id
        }, 
        // Create chat id if doesn't exist
        None => {
            debug!("Message was sent to: {}. Chat with this user wasn't found. Creating chat.", msg.common.to);

            if !users_db.exists(&msg.common.to.into()).await? {
                return Err(PPError::from("Target user_id doesn't exist!"));
            }

            let chat_id = handler.get_db::<ChatsDB>()
                .create_private(vec![self_user_id.clone(), msg.common.to.into()])
                .await
                .unwrap();
            users_db.add_chat(&self_user_id, msg.common.to, chat_id.chat_id()).await.unwrap();
            users_db.add_chat(&msg.common.to.into(), self_user_id.as_i32_unchecked(), chat_id.chat_id()).await.unwrap();

            let mut chat_details = chat_id.details(&msg.common.to.into()).await?.unwrap();
            chat_details.chat_id = self_user_id.as_i32().unwrap();
            handler.send_msg_to_connection_detached(msg.common.to, NewChatEvent {
                event: "new_chat".into(),
                new_chat: chat_details
            });

            chat_id.chat_id()
        }
    };
    
    let messages_db: MessagesDB = handler.get_db();
    let mut db_message = messages_db.add_message(&msg, &self_user_id, associated_chat_id).await?;
    db_message.chat_id = msg.common.to;
    let message_id = db_message.message_id;

    handler.send_msg_to_connection_detached(msg.common.to, NewMessageEvent {
        event: "new_message".into(),
        new_message: db_message
    });

    Ok((message_id, msg.common.to))
}

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler.send_error(method, "You aren't authenticated!".into()).await;
            return;
        }
    }

    let content = handler.utf8_content_unchecked();
    match serde_json::from_str::<MessageRequest>(&content) {
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
