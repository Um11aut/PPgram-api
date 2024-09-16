
use crate::{db::user::USERS_DB, server::message::{handler::MessageHandler, types::{request::check::*, response::check::CheckResponseMessage}}};

async fn check_username(username: &str, handler: &mut MessageHandler) {
    match USERS_DB.get().unwrap().exists(&username.into()).await {
        Ok(exists) => {
            let data = if exists {
                CheckResponseMessage {
                    ok: true,
                    method: "check_username".into()
                }
            } else {
                CheckResponseMessage {
                    ok: false,
                    method: "check_username".into()
                }
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
    match serde_json::from_str::<CheckUsernameRequestMessage>(handler.builder.as_mut().unwrap().content_utf8().unwrap()) {
        Ok(msg) => {
            check_username(&msg.data, handler).await;
        },
        Err(err) => {
            handler.send_error(method, err.to_string().into()).await;
        },
    }
}