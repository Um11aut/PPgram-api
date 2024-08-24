use std::sync::Arc;

use serde_json::json;
use tokio::io::AsyncWriteExt;

use crate::{db::user::USERS_DB, server::message::{builder::MessageBuilder, handler::MessageHandler, types::{error::error::PPErrorSender, fetch::check::CheckUsernameRequestMessage}}};

async fn check_username(username: &str, handler: &mut MessageHandler) {
    match USERS_DB.get().unwrap().exists(username.into()).await {
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

            handler.send_message(&data).await;
        }
        Err(err) => {
            handler.send_error("check_username", err).await;
        }
    }
}

pub async fn handle(handler: &mut MessageHandler, method: &str) 
{
    match serde_json::from_str::<CheckUsernameRequestMessage>(handler.builder.as_ref().unwrap().content()) {
        Ok(msg) => {
            check_username(&msg.data, handler).await;
        },
        Err(err) => {
            handler.send_err_str(method, err.to_string()).await;
        },
    }
}