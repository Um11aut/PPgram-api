use log::{debug, error, info};
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::{
    db::{
        chat::{chats::CHATS_DB, messages::MESSAGES_DB},
        internal::error::PPError,
        user::USERS_DB,
    },
    server::{
        message::{
            builder::Message,
            handler::RequestMessageHandler,
            types::{error::error::PPErrorSender, message::RequestMessage},
        },
        server::Connections,
        session::Session,
    },
};
use std::sync::Arc;

/// This function fetches chat_id by the given target user_id
/// 
/// It goes through all the registered to user chats and finds the target id 
/// 
/// TODO: Fully remake this system, because it may significantly affect performance
async fn find_chat_id(session: &Session, target_user_id: i32) -> Result<Option<i32>, PPError> {
    let users_db = USERS_DB.get().unwrap();
    let (self_user_id, _) = session.get_credentials().unwrap();
    let chat_ids = users_db.fetch_chats(self_user_id).await?;

    let chats_db = CHATS_DB.get().unwrap();
    for chat_id in chat_ids {
        let chat_info = chats_db.fetch_chat_info(chat_id).await?;

        if chat_info.participants.iter().any(|&participant| participant == target_user_id) {
            return Ok(Some(chat_id));
        }
    }

    Ok(None)
}

async fn handle_send_message(
    session: &Session,
    msg: RequestMessage,
    connections: Connections,
) -> Result<(), PPError> {
    let (user_id, _) = session.get_credentials().unwrap();

    if user_id == msg.common.to {
        return Err(PPError::from("You cannot send messages on yourself!"));
    }

    let target_chat_id = match find_chat_id(session, msg.common.to).await? {
        Some(existing_chat_id) => {
            existing_chat_id
        }, 
        // Create chat id if doesn't exist
        None => {
            debug!("Message was sent to: {}. Chat with this user wasn't found. Creating chat.", msg.common.to);
            let users_db = USERS_DB.get().unwrap();

            if !users_db.user_id_exists(msg.common.to).await? {
                return Err(PPError::from("Target user_id doesn't exist!"));
            }

            let chat_id = CHATS_DB
                .get()
                .unwrap()
                .create_chat(vec![user_id, msg.common.to])
                .await
                .unwrap();
            users_db.add_chat(user_id, chat_id).await.unwrap();
            users_db.add_chat(msg.common.to, chat_id).await.unwrap();
            chat_id
        }
    };
    
    let messages_db = MESSAGES_DB.get().unwrap();
    messages_db.add_message(&msg, user_id, target_chat_id).await?;

    {
        if let Some(reciever_session) = connections.write().await.get(&msg.common.to) {
            let target_connection = reciever_session.lock().await;
            target_connection.send(serde_json::to_string(&msg).unwrap()).await;
        }
    }

    Ok(())
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) {
    let session = handler.session.lock().await;
    if !session.is_authenticated() {
        PPErrorSender::send(
            method,
            "You aren't authenticated!",
            Arc::clone(&handler.writer),
        )
        .await;
        return;
    }

    match serde_json::from_str::<RequestMessage>(handler.builder.clone().unwrap().content()) {
        Ok(msg) => match msg.common.method.as_str() {
            "send_message" => {
                match handle_send_message(&session, msg, Arc::clone(&handler.connections)).await {
                    Ok(_) => {
                        let data = serde_json::json!({ "method": "send_message", "ok": true });
                        handler
                            .writer
                            .lock()
                            .await
                            .write_all(
                                Message::build_from(serde_json::to_string(&data).unwrap())
                                    .packed()
                                    .as_bytes(),
                            )
                            .await
                            .unwrap();
                    }
                    Err(err) => {err.safe_send(method, Arc::clone(&handler.writer)).await;},
                }
            }
            _ => PPErrorSender::send(method, "Unknown method!", Arc::clone(&handler.writer)).await,
        },
        Err(err) => {
            PPErrorSender::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }
}
