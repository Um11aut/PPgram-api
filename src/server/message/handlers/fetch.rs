use log::{error, info};
use serde::de;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::{db::{chat::chats::CHATS_DB, internal::error::PPError, user::USERS_DB}, server::message::{builder::Message, handler::RequestMessageHandler, types::{chat::{Chat, ChatDetails, ResponseChatsDetails}, error::error::PPErrorSender, user::{ResponseUserDetails, UserDetails}}}};
use crate::server::session::Session;
use std::sync::Arc;

async fn fetch_chats(handler: &RequestMessageHandler) -> Option<Vec<ChatDetails>>{
    let users_db = USERS_DB.get().unwrap();
    let chats_db = CHATS_DB.get().unwrap();

    let (user_id, _) = handler.session.lock().await.get_credentials().unwrap();
    return match users_db.fetch_chats(user_id).await {
        Ok(chat_ids) => {
            let mut chats_details: Vec<ChatDetails> = vec![];
            for chat_id in chat_ids {
                let chat = chats_db.fetch_chat_info(chat_id).await.unwrap();
                let info = chats_db.fetch_chat_details(user_id, &chat).await.unwrap()?;
                chats_details.push(info);
            }
            Some(chats_details)
        }
        Err(err) => {
            err.safe_send("fetch_chats", Arc::clone(&handler.writer)).await;
            None
        }
    }
}

async fn fetch_self(handler: &RequestMessageHandler) -> Option<UserDetails> {
    let (user_id, _) = handler.session.lock().await.get_credentials()?;
    match USERS_DB.get().unwrap().fetch_user(user_id).await {
        Ok(details) => return details,
        Err(err) => {
            err.safe_send("fetch_self", Arc::clone(&handler.writer)).await;
            return None
        }
    }
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) 
{
    {
        let session = handler.session.lock().await;
        if !session.is_authenticated() {
            PPErrorSender::send(method, "You aren't authenticated!", Arc::clone(&handler.writer)).await;
            return;
        }
    }

    match serde_json::from_str::<Value>(handler.builder.clone().unwrap().content()) {
        Ok(value) => {
            let what = value.get("what");
            if let Some(what) = what {
                let what = what.as_str();
                if let Some(what) = what {
                    let response: Option<String> = match what {
                        "chats" => {
                            let res = fetch_chats(&handler).await;
                            if let Some(chats) = res {
                                let details = ResponseChatsDetails {
                                    method: "fetch_chats".into(),
                                    response: chats  
                                };
                                let details = serde_json::to_string(&details).unwrap();
                                Some(details)
                            } else {
                                PPErrorSender::send(method, "Failed to retrieve chat details!", Arc::clone(&handler.writer)).await;
                                None
                            }
                        }
                        "self" => {
                            let res = fetch_self(&handler).await;
                            if let Some(profile) = res {
                                let details = ResponseUserDetails {
                                    method: "fetch_self".into(),
                                    response: profile
                                };
                                let details = serde_json::to_string(&details).unwrap();
                                Some(details)
                            } else {
                                PPErrorSender::send(method, "Failed to retrieve self details!", Arc::clone(&handler.writer)).await;
                                None
                            }
                        }
                        _ => {
                            PPErrorSender::send(method, "Unknown 'what' field!", Arc::clone(&handler.writer)).await;
                            None
                        }
                    };

                    if let Some(response) = response {
                        handler.writer.lock().await.write_all(Message::build_from(response).packed().as_bytes()).await.unwrap();
                    }
                } else {
                    PPErrorSender::send(method, "'what' field must be string!", Arc::clone(&handler.writer)).await;
                }
            } else {
                PPErrorSender::send(method, "Failed to get 'what' field!", Arc::clone(&handler.writer)).await;
            }
        },
        Err(err) => {
            PPErrorSender::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }     
}
