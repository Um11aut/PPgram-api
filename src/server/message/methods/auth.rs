use std::{future::Future, sync::Arc};

use crate::{
    db::internal::error::PPError,
    server::{
        message::{
            builder::MessageBuilder,
            handler::MessageHandler,
            types::{
                error::error::PPErrorSender, request::auth::*,
            },
        },
        session::Session,
    },
};
use log::error;
use serde_json::json;
use tokio::io::AsyncWriteExt;

async fn handle_auth_message<'a, T, Fut>(
    buffer: &str,
    session: &'a mut Session,
    handler: impl FnOnce(&'a mut Session, T) -> Fut,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
    Fut: Future<Output = Result<(), PPError>>
{
    if session.is_authenticated() {
        return Err(Box::new(PPError::from(
            "You are already authenticated!",
        )));
    }

    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => match handler(session, auth_message).await {
            Ok(()) => {}
            Err(err) => match err {
                PPError::Cassandra(internal_err) => {
                    error!("{}", internal_err);
                    return Err(Box::new(PPError::from("Internal error.")));
                }
                PPError::Client(_) => {
                    return Err(Box::new(err));
                }
            },
        },
        Err(err) => return Err(Box::new(err)),
    }

    Ok(())
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    let builder = handler.builder.as_ref().unwrap();
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
        PPErrorSender::send(method, err.to_string(), Arc::clone(&handler.writer)).await;
        return;
    }


    if let Some((user_id, session_id)) = session.get_credentials() {
        let user_id = user_id.get_i32().unwrap();
        {
            let mut connections = handler.connections.write().await;
            connections.insert(user_id, Arc::clone(&handler.session));
        }

        let data = match method {
            "auth" => json!({ "method": method, "ok": true }),
            _ => json!({ "method": method, "ok": true, "user_id": user_id, "session_id": session_id }),
        };
        handler
            .writer
            .lock()
            .await
            .write_all(
                MessageBuilder::build_from(serde_json::to_string(&data).unwrap())
                    .packed()
                    .as_bytes(),
            )
            .await
            .unwrap();
    };
}
