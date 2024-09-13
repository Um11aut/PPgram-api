use log::debug;
use serde_json::json;
use tokio::sync::RwLock;

use crate::{
    db::{
        chat::{self, chats::CHATS_DB, messages::MESSAGES_DB},
        internal::error::PPError,
        user::USERS_DB,
    },
    server::{
        message::{
            handler::MessageHandler, types::{chat::ChatId, request::message::{Message, MessageId}}
        },
        session::Session,
    },
};
use std::sync::Arc;

/// Returns latest chat message id if sucessful
async fn handle_send_message(
    session: Arc<RwLock<Session>>,
    msg: Message,
    handler: &MessageHandler,
) -> Result<(MessageId, ChatId), PPError> {
    let session = session.read().await;
    let (self_user_id, _) = session.get_credentials().unwrap();
    drop(session);

    if self_user_id.get_i32().unwrap() == msg.common.to {
        return Err(PPError::from("You cannot send messages on yourself!"));
    }

    let maybe_chat = USERS_DB.get().unwrap().get_associated_chat_id(&self_user_id, &msg.common.to.into()).await?;
    let associated_chat_id = match maybe_chat {
        Some(existing_chat_id) => {
            existing_chat_id
        }, 
        // Create chat id if doesn't exist
        None => {
            debug!("Message was sent to: {}. Chat with this user wasn't found. Creating chat.", msg.common.to);
            let users_db = USERS_DB.get().unwrap();

            if !users_db.exists(&msg.common.to.into()).await? {
                return Err(PPError::from("Target user_id doesn't exist!"));
            }

            let chat_id = CHATS_DB
                .get()
                .unwrap()
                .create_chat(vec![self_user_id.clone(), msg.common.to.into()])
                .await
                .unwrap();
            users_db.add_chat(&self_user_id, &msg.common.to.into(), chat_id.chat_id()).await.unwrap();
            users_db.add_chat(&msg.common.to.into(), &self_user_id, chat_id.chat_id()).await.unwrap();

            let mut chat_details = chat_id.details(&msg.common.to.into()).await?.unwrap();
            chat_details.chat_id = self_user_id.get_i32().unwrap();
            handler.send_msg_to_connection(msg.common.to, json!({
                "event": "new_chat",
                "ok": true,
                "data": chat_details
            }));

            chat_id.chat_id()
        }
    };
    
    let messages_db = MESSAGES_DB.get().unwrap();
    let mut db_message = messages_db.add_message(&msg, &self_user_id, associated_chat_id).await?;
    db_message.chat_id = msg.common.to;
    let message_id = db_message.message_id;

    handler.send_msg_to_connection(msg.common.to, json!({
        "event": "new_message",
        "ok": true,
        "data": db_message
    }));

    Ok((message_id, msg.common.to))
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler.send_error(method, "You aren't authenticated!".into()).await;
            return;
        }
    }

    match serde_json::from_str::<Message>(handler.builder.as_mut().unwrap().content_utf8().unwrap()) {
        Ok(msg) => match msg.common.method.as_str() {
            "send_message" => {
                match handle_send_message(Arc::clone(&handler.session), msg, &handler).await {
                    Ok((latest_msg_id, target_chat_id)) => {
                        let data = serde_json::json!({ "method": "send_message", "message_id": latest_msg_id, "chat_id": target_chat_id, "ok": true });
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
