use log::{error, info};
use serde::de;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::{db::{chat::chats::CHATS_DB, internal::error::DatabaseError, user::USERS_DB}, server::message::{builder::Message, handler::RequestMessageHandler, types::{chat::{Chat, ChatDetails, ResponseChatsDetails}, error::error::PPgramError, user::UserInfo}}};
use crate::server::session::Session;
use std::sync::Arc;

async fn fetch_chats(handler: &RequestMessageHandler) -> Option<Vec<ChatDetails>>{
    let users_db = USERS_DB.get().unwrap();
    let chats_db = CHATS_DB.get().unwrap();

    let (user_id, _) = handler.session.lock().await.get_credentials().unwrap();
    match users_db.fetch_chats(user_id).await {
        Ok(chat_ids) => {
            let mut chats_details: Vec<ChatDetails> = vec![];
            for chat_id in chat_ids {
                let chat = chats_db.fetch_chat_info(chat_id).await.unwrap();
                let info = chats_db.fetch_chat_details(user_id, &chat).await.unwrap()?;
                chats_details.push(info);
            }
            return Some(chats_details)
        }
        Err(err) => {
            err.safe_send("fetch", Arc::clone(&handler.writer)).await;
        }
    }
    None
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) 
{
    {
        if !handler.session.lock().await.is_authenticated() {
            PPgramError::send(method, "You aren't authenticated!", Arc::clone(&handler.writer)).await;
            return;
        }
    }

    match serde_json::from_str::<Value>(handler.builder.clone().unwrap().content()) {
        Ok(value) => {
            let what = value.get("what");
            if let Some(what) = what {
                let what = what.as_str();
                if let Some(what) = what {
                    match what {
                        "chats" => {
                            let res = fetch_chats(&handler).await;
                            if let Some(chats) = res {
                                let details = ResponseChatsDetails {
                                    method: method.into(),
                                    chats  
                                };
                                let details = serde_json::to_string(&details).unwrap();
                                handler.writer.lock().await.write_all(Message::build_from(details).packed().as_bytes()).await.unwrap();
                            } else {
                                PPgramError::send(method, "Failed to fetch chat details!", Arc::clone(&handler.writer)).await;
                            }
                        }
                        _ => {
                            PPgramError::send(method, "Unknown 'what' field!", Arc::clone(&handler.writer)).await;
                        }
                    }
                } else {
                    PPgramError::send(method, "'what' field must be string!", Arc::clone(&handler.writer)).await;
                }
            } else {
                PPgramError::send(method, "Failed to get 'what' field!", Arc::clone(&handler.writer)).await;
            }
        },
        Err(err) => {
            PPgramError::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }     
}
