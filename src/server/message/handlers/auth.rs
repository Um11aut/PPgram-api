use std::{future::Future, sync::Arc};

use crate::server::{
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
};
use serde::de::Error;
use serde_json::json;
use tokio::io::AsyncWriteExt;

async fn handle_auth_message<'a, T, F, Fut>(
    buffer: &str,
    session: &'a mut Session,
    handler: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(&'a mut Session, T) -> Fut,
    Fut: Future<Output = Result<(), cassandra_cpp::Error>>,
{
    if session.is_authenticated() {
        return Err(Box::new(serde_json::Error::custom(
            "You are already authenticated!",
        )));
    }

    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => match handler(session, auth_message).await {
            Ok(()) => {}
            Err(err) => {
                return Err(Box::new(serde_json::Error::custom(err)));
            }
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
            handle_auth_message::<RequestLoginMessage, _, _>(
                buffer.as_str(),
                &mut session,
                Session::login,
            )
            .await
        }
        "auth" => {
            handle_auth_message::<RequestAuthMessage, _, _>(
                buffer.as_str(),
                &mut session,
                Session::auth,
            )
            .await
        }
        "register" => {
            handle_auth_message::<RequestRegisterMessage, _, _>(
                buffer.as_str(),
                &mut session,
                Session::register,
            )
            .await
        }
        _ => Ok(()),
    };

    if let Some((user_id, session_id)) = session.get_credentials() {
        let data = json!({ "ok": true, "user_id": user_id, "session_id": session_id });
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

    if let Err(err) = res {
        PPgramError::send(err.to_string(), Arc::clone(&handler.writer)).await;
    }
}
