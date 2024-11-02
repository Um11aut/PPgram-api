
use crate::{db::user::UsersDB, server::message::{handlers::json_handler::TCPHandler, types::{request::check::*, response::check::CheckResponse}}};

async fn check_username(handler: &mut TCPHandler, username: &str) {
    let users_db: UsersDB = handler.get_db();

    match users_db.exists(&username.into()).await {
        Ok(exists) => {
            let data = if exists {
                CheckResponse {
                    ok: true,
                    method: "check_username".into()
                }
            } else {
                CheckResponse {
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

pub async fn handle(handler: &mut TCPHandler, method: &str) 
{
    match serde_json::from_str::<CheckUsernameRequest>(&handler.utf8_content_unchecked()) {
        Ok(msg) => {
            check_username(handler, &msg.data).await;
        },
        Err(err) => {
            handler.send_error(method, err.to_string().into()).await;
        },
    }
}