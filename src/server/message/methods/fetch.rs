use log::{error, info};
use serde::de;
use serde_json::{json, Value};
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::fetch::fetch::BaseFetchRequestMessage;
use crate::server::session::Session;
use crate::{
    db::{
        chat::chats::CHATS_DB,
        internal::error::PPError,
        user::{self, USERS_DB},
    },
    server::message::{
        builder::MessageBuilder,
        handler::RequestMessageHandler,
        types::{
            error::error::PPErrorSender,
            user::{User, UserId},
        },
    },
};
use std::sync::Arc;

async fn handle_fetch_chats(handler: &RequestMessageHandler) -> Option<Vec<ChatDetails>> {
    let users_db = USERS_DB.get().unwrap();
    let chats_db = CHATS_DB.get().unwrap();

    let (user_id, _) = handler.session.lock().await.get_credentials().unwrap();
    return match users_db.fetch_chats(&user_id).await {
        Ok(chat_ids) => {
            let mut chats_details: Vec<ChatDetails> = vec![];
            for chat_id in chat_ids {
                let chat = chats_db.fetch_chat(chat_id).await.unwrap();
                if let Some(chat) = chat {
                    let details = chat.details(&user_id).await;

                    match details {
                        Ok(details) => {
                            if let Some(details) = details {
                                chats_details.push(details);
                            }
                        }
                        Err(err) => {
                            err.safe_send("fetch_chats", Arc::clone(&handler.writer));
                        }
                    }
                }
            }
            Some(chats_details)
        }
        Err(err) => {
            err.safe_send("fetch_chats", Arc::clone(&handler.writer))
                .await;
            None
        }
    };
}

async fn fetch_user(
    method: &str,
    handler: &RequestMessageHandler,
    identifier: UserId,
) -> Option<User> {
    match USERS_DB.get().unwrap().fetch_user(identifier).await {
        Ok(details) => return details,
        Err(err) => {
            err.safe_send(method, Arc::clone(&handler.writer)).await;
            return None;
        }
    }
}

async fn handle_fetch_self(handler: &RequestMessageHandler) -> Option<User> {
    let (user_id, _) = handler.session.lock().await.get_credentials()?;
    fetch_user("fetch_user", handler, user_id.into()).await
}

async fn handle_fetch_user(username: &str, handler: &RequestMessageHandler) -> Option<User> {
    match USERS_DB.get().unwrap().exists(username.into()).await {
        Ok(exists) => {
            if exists {
                return fetch_user("fetch_user", handler, username.into()).await;
            }
        }
        Err(err) => {
            err.safe_send("fetch_user", Arc::clone(&handler.writer))
                .await;
        }
    }

    None
}

// async fn handle_fetch_message(handler: &RequestMessageHandler)

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) {
    {
        let session = handler.session.lock().await;
        if !session.is_authenticated() {
            handler.send_error(method, "You aren't authenticated!").await;
        }
    }

    // "user" => {
    //                 let username: Option<Option<&str>> = base_fetch_msg.get("username").map(|v| v.as_str());
    //                 if let Some(Some(username)) = username {
    //                     handle_fetch_user(username, &handler)
    //                         .await
    //                         .map(|v| ResponseUserDetails::to_json_string("fetch_user", v))
    //                 } else {
    //                     None
    //                 }
    //             }

    match serde_json::from_str::<BaseFetchRequestMessage>(handler.builder.as_ref().unwrap().content()) {
        Ok(base_fetch_msg) => {
            let response: Option<Value> = match base_fetch_msg.what.as_str() {
                "chats" => handle_fetch_chats(&handler).await.map(|chats| {
                    let details = json!({
                        "ok": true,
                        "method": "fetch_chats",
                        "data": chats,
                    });
                    serde_json::to_value(details).unwrap()
                }),
                "self" => handle_fetch_self(&handler)
                    .await
                    .map(|v| {
                        v.build_response("fetch_self")
                    }),
                _ => {
                    handler.send_error(method, "Unknown 'what' field!").await;
                    return;
                }
            };

            if let Some(response) = response {
                handler.send_message(&response).await;
            }
            return;
        }
        Err(err) => {
            PPErrorSender::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
            return;
        }
    }
}
