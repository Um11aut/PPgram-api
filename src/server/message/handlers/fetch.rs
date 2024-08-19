use log::{error, info};
use serde::de;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, sync::Mutex};

use crate::server::session::Session;
use crate::{
    db::{
        chat::chats::CHATS_DB,
        internal::error::PPError,
        user::{self, USERS_DB},
    },
    server::message::{
        builder::Message,
        handler::RequestMessageHandler,
        types::{
            chat::{Chat, ChatDetails, ResponseChatsDetails},
            error::error::PPErrorSender,
            user::{ResponseUserDetails, UserDetails, UserIdentifier},
        },
    },
};
use std::sync::Arc;

async fn handle_fetch_chats(handler: &RequestMessageHandler) -> Option<Vec<ChatDetails>> {
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
            err.safe_send("fetch_chats", Arc::clone(&handler.writer))
                .await;
            None
        }
    };
}

async fn fetch_user(
    method: &str,
    handler: &RequestMessageHandler,
    identifier: UserIdentifier,
) -> Option<UserDetails> {
    match USERS_DB.get().unwrap().fetch_user(identifier).await {
        Ok(details) => return details,
        Err(err) => {
            err.safe_send(method, Arc::clone(&handler.writer)).await;
            return None;
        }
    }
}

async fn handle_fetch_self(handler: &RequestMessageHandler) -> Option<UserDetails> {
    let (user_id, _) = handler.session.lock().await.get_credentials()?;
    fetch_user("fetch_user", handler, user_id.into()).await
}

async fn handle_fetch_user(username: &str, handler: &RequestMessageHandler) -> Option<UserDetails> {
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

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) {
    {
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
    }

    match serde_json::from_str::<Value>(handler.builder.as_ref().unwrap().content()) {
        Ok(value) => {
            let what = value.get("what").and_then(|val| val.as_str());
            if let Some(what) = what {
                let response: Option<String> = match what {
                    "chats" => handle_fetch_chats(&handler).await.map(|chats| {
                        let details = ResponseChatsDetails {
                            method: "fetch_chats".into(),
                            response: chats,
                        };
                        serde_json::to_string(&details).unwrap()
                    }),
                    "self" => handle_fetch_self(&handler)
                        .await
                        .map(|v| ResponseUserDetails::to_json_string("fetch_self", v)),
                    "user" => {
                        let username = value.get("username").map(|v| v.as_str());
                        if let Some(Some(username)) = username {
                            handle_fetch_user(username, &handler)
                                .await
                                .map(|v| ResponseUserDetails::to_json_string("fetch_user", v))
                        } else {
                            None
                        }
                    }
                    _ => {
                        PPErrorSender::send(
                            method,
                            "Unknown 'what' field!",
                            Arc::clone(&handler.writer),
                        )
                        .await;
                        None
                    }
                };

                if let Some(response) = response {
                    handler
                        .writer
                        .lock()
                        .await
                        .write_all(Message::build_from(response).packed().as_bytes())
                        .await
                        .unwrap();
                } else {
                    PPErrorSender::send(
                        method,
                        "Failed to find information by the given information!",
                        Arc::clone(&handler.writer),
                    )
                    .await;
                }
            } else {
                PPErrorSender::send(
                    method,
                    "Failed to parse method 'what'",
                    Arc::clone(&handler.writer),
                )
                .await;
            }
        }
        Err(err) => {
            PPErrorSender::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }
}
