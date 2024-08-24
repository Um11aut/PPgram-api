use log::{error, info};
use serde::de;
use serde_json::{json, Value};
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::db::chat::messages::MESSAGES_DB;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::fetch::fetch::{BaseFetchRequestMessage, FetchMessagesRequestMessage};
use crate::server::message::types::request::message::{DbMesssage, Message};
use crate::server::session::Session;
use crate::{
    db::{
        chat::chats::CHATS_DB,
        internal::error::PPError,
        user::{self, USERS_DB},
    },
    server::message::{
        builder::MessageBuilder,
        handler::MessageHandler,
        types::{
            error::error::PPErrorSender,
            user::{User, UserId},
        },
    },
};
use std::sync::Arc;

async fn handle_fetch_chats(handler: &MessageHandler) -> Option<Vec<ChatDetails>> {
    let users_db = USERS_DB.get().unwrap();
    let chats_db = CHATS_DB.get().unwrap();

    let (user_id, _) = handler.session.read().await.get_credentials().unwrap();
    return match users_db.fetch_chats(&user_id).await {
        Ok(chat_ids) => {
            let mut chats_details: Vec<ChatDetails> = vec![];
            for (chat_id, associated_chat_id) in chat_ids {
                let chat = chats_db.fetch_chat(associated_chat_id).await.unwrap();
                if let Some(chat) = chat {
                    let details = chat.details(&user_id, chat_id).await;

                    match details {
                        Ok(details) => {
                            if let Some(details) = details {
                                chats_details.push(details);
                            }
                        }
                        Err(err) => {
                            handler.send_error("fetch_chats", err).await;
                        }
                    }
                }
            }
            Some(chats_details)
        }
        Err(err) => {
            handler.send_error("fetch_chats", err).await;
            None
        }
    };
}

async fn fetch_user(
    method: &str,
    handler: &MessageHandler,
    identifier: UserId,
) -> Option<User> {
    match USERS_DB.get().unwrap().fetch_user(identifier).await {
        Ok(details) => return details,
        Err(err) => {
            handler.send_error(method, err).await;
            return None;
        }
    }
}

async fn handle_fetch_self(handler: &MessageHandler) -> Option<User> {
    let (user_id, _) = handler.session.read().await.get_credentials()?;
    fetch_user("fetch_user", handler, user_id.into()).await
}

async fn handle_fetch_user(username: &str, handler: &MessageHandler) -> Option<User> {
    match USERS_DB.get().unwrap().exists(username.into()).await {
        Ok(exists) => {
            if exists {
                return fetch_user("fetch_user", handler, username.into()).await;
            }
        }
        Err(err) => {
            handler.send_error("fetch_user", err).await;
        }
    }

    None
}

async fn handle_fetch_messages(handler: &MessageHandler, msg: FetchMessagesRequestMessage) -> Option<Vec<DbMesssage>> {
    let res = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials().unwrap();
        USERS_DB.get().unwrap().get_associated_chat_id(&user_id, &msg.chat_id.into()).await
    };

    match res {
        Ok(chat_id) => {
            if let Some(target_chat_id) = chat_id {
                match MESSAGES_DB.get().unwrap().fetch_messages(target_chat_id, msg.range[0]..msg.range[1]).await {
                    Ok(maybe_messages) => {
                        match maybe_messages {
                            Some(mut messages) => {
                                messages.iter_mut().for_each(|message| message.chat_id = msg.chat_id);
                                return Some(messages)
                            }
                            None => return None
                        }
                    }
                    Err(err) => {
                        handler.send_error("fetch_messages", err).await;
                    }
                }
            }
            
        }
        Err(err) => {
            handler.send_error("fetch_messages", err).await;
        }
    };
    
    None
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler.send_err_str(method, "You aren't authenticated!").await;
            return;
        }
    }

    match serde_json::from_str::<BaseFetchRequestMessage>(handler.builder.as_ref().unwrap().content()) {
        Ok(base_fetch_msg) => {
            let response: Option<Value> = match base_fetch_msg.what.as_str() {
                "chats" => handle_fetch_chats(&handler).await.map(|chats| {
                    let details = json!({
                        "ok": true,
                        "method": "fetch_chats",
                        "data": if chats.is_empty() {None} else { Some(chats)},
                    });
                    serde_json::to_value(details).unwrap()
                }),
                "self" => handle_fetch_self(&handler)
                    .await
                    .map(|v| {
                        v.build_response("fetch_self")
                    }),
                "user" => {
                    let value = serde_json::from_str::<Value>(&handler.builder.as_ref().unwrap().content());

                    match value {
                        Ok(value) => {
                            let username: Option<Option<&str>> = value.get("username").map(|v| v.as_str());
                            if let Some(Some(username)) = username {
                                handle_fetch_user(username, &handler)
                                    .await
                                    .map(|v| v.build_response("fetch_user"))
                            } else {
                                None
                            }
                        }
                        Err(err) => {
                            handler.send_err_str("fetch_user", err.to_string()).await;
                            None
                        }
                    }
                }
                "messages" => {
                    let value = serde_json::from_str::<FetchMessagesRequestMessage>(&handler.builder.as_ref().unwrap().content());

                    match value {
                        Ok(msg) => {
                            let out = handle_fetch_messages(&handler, msg).await;
                            let response = json!({
                                "method": "fetch_messages",
                                "ok": true,
                                "data": out
                            });

                            Some(response)
                        }
                        Err(err) => {
                            handler.send_err_str("fetch_messages", err.to_string()).await;
                            None
                        }
                    }
                }
                _ => {
                    handler.send_err_str(method, "Unknown 'what' field!").await;
                    return;
                }
            };

            if let Some(response) = response {
                handler.send_message(&response).await;
            }
            return;
        }
        Err(err) => {
            handler.send_err_str(method, err.to_string()).await;
            return;
        }
    }
}
