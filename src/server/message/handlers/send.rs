use log::{error, info};
use serde::Serialize;
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::{db::{chat::{chats::CHATS_DB, messages::MESSAGES_DB}, internal::error::DatabaseError, user::USERS_DB}, server::{message::{builder::Message, handler::RequestMessageHandler, types::{error::error::PPgramError, message::RequestMessage}}, server::Connections, session::{self, Session}}};
use std::sync::Arc;

async fn chat_exists(session: &Session, chat_id: i32) -> Result<bool, DatabaseError> {
    let (user_id, _) = session.get_credentials().unwrap();
    let users_db = USERS_DB.get().unwrap();

    let chats = users_db.fetch_chats(user_id).await?;

    Ok(chats.iter().any(|&chat| {chat == chat_id}))
}

async fn handle_send_message(session: &Session, msg: RequestMessage, connections: Connections) -> Result<(), DatabaseError> {
    let chat_exists = chat_exists(&session, msg.common.to).await?;

    let (user_id, _) = session.get_credentials().unwrap();
    if !chat_exists {   
        let users_db = USERS_DB.get().unwrap(); 

        if !users_db.user_id_exists(msg.common.to).await? {
            return Err(DatabaseError::from("User Id doesn't exist!"))
        }

        let chat_id = CHATS_DB.get().unwrap().create_chat(vec![user_id, msg.common.to]).await.unwrap();
        users_db.add_chat(user_id, chat_id).await.unwrap();
    }

    let messages_db = MESSAGES_DB.get().unwrap();
    messages_db.add_message(&msg, user_id).await?;

    {
        if let Some(reciever_session) = connections.write().await.get(&msg.common.to) {
            let reciever_session = reciever_session.lock().await;
            
            // reciever_session.send(serde_json::from_value(msg.).unwrap()).await;
        }
    }

    Ok(())
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) 
{
    let session = handler.session.lock().await;
    if !session.is_authenticated() {
        PPgramError::send(method, "You aren't authenticated!", Arc::clone(&handler.writer)).await;
        return;
    }

    match serde_json::from_str::<RequestMessage>(handler.builder.clone().unwrap().content()) {
        Ok(msg) => {
            match msg.common.method.as_str() {
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
                        },
                        Err(err) => match err {
                            DatabaseError::Cassandra(internal) => {
                                error!("{}", internal);
                                PPgramError::send(method, "Internal error!", Arc::clone(&handler.writer)).await;
                            },
                            DatabaseError::Client(_) => {
                                PPgramError::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
                            }
                        },
                    }
                }
                _ => PPgramError::send(method, "Unknown method!", Arc::clone(&handler.writer)).await
            }
        }
        Err(err) => {
            PPgramError::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }
}
