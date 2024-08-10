use std::{future::Future, sync::Arc};

use crate::{
    db::internal::error::DatabaseError,
    server::{
        message::{
            builder::Message,
            handler::RequestMessageHandler,
            types::{
                authentication::message::{
                    RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage,
                },
                error::error::PPgramError,
            },
        },
        session::Session,
    },
};
use log::{debug, error};
use serde::de::Error;
use serde_json::json;
use tokio::io::AsyncWriteExt;

async fn handle_auth_message<'a, T, Fut>(
    buffer: &str,
    session: &'a mut Session,
    handler: impl FnOnce(&'a mut Session, T) -> Fut,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
    Fut: Future<Output = Result<(), DatabaseError>>
{
    if session.is_authenticated() {
        return Err(Box::new(serde_json::Error::custom(
            "You are already authenticated!",
        )));
    }

    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => match handler(session, auth_message).await {
            Ok(()) => {}
            Err(err) => match err {
                DatabaseError::Cassandra(internal_err) => {
                    error!("{}", internal_err);
                    return Err(Box::new(DatabaseError::from("Internal error.")));
                }
                DatabaseError::Client(_) => {
                    return Err(Box::new(err));
                }
            },
        },
        Err(err) => return Err(Box::new(err)),
    }

    Ok(())
}

pub async fn handle(handler: &mut RequestMessageHandler, method: &str) {
    let builder = handler.builder.clone().unwrap();
    let buffer = builder.content();

    let mut session = handler.session.lock().await;

    let res = match method {
        "login" => {
            handle_auth_message::<RequestLoginMessage, _>(
                buffer.as_str(),
                &mut session,
                Session::login,
            )
            .await
        }
        "auth" => {
            handle_auth_message::<RequestAuthMessage, _>(
                buffer.as_str(),
                &mut session,
                Session::auth,
            )
            .await
        }
        "register" => {
            handle_auth_message::<RequestRegisterMessage, _>(
                buffer.as_str(),
                &mut session,
                Session::register,
            )
            .await
        }
        _ => Ok(()),
    };

    if let Err(err) = res {
        PPgramError::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        return;
    }

    if let Some((user_id, session_id)) = session.get_credentials() {
        let data = if method != "auth" {
            json!({ "method": method, "ok": true, "user_id": user_id, "session_id": session_id })
        } else {
            json!({"ok": true})
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
    };
}
