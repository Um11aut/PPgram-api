use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::{db::user::USERS_DB, server::message::{builder::Message, handler::RequestMessageHandler, types::{error::error::PPgramError, fetch::check::CheckUsernameRequestMessage}}};

async fn check_username(username: &str, handler: &mut RequestMessageHandler) {
    match USERS_DB.get().unwrap().username_exists(username).await {
        Ok(exists) => {
            let data = if exists {
                json!({
                    "method": "check_username",
                    "ok": true
                })
            } else {
                json!({
                    "method": "check_username",
                    "ok": false,
                })
            };

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
        Err(err) => {
            PPgramError::send("check", err.to_string(), Arc::clone(&handler.writer)).await;
        }
    }
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) 
{
    match serde_json::from_str::<CheckUsernameRequestMessage>(handler.builder.clone().unwrap().content()) {
        Ok(msg) => {
            check_username(&msg.data, handler).await;
        },
        Err(err) => {
            PPgramError::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        },
    }
}