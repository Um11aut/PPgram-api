use std::{future::Future, sync::Arc};

use crate::{
    db::internal::error::PPError,
    server::{
        message::{
            handler::MessageHandler,
            types::request::auth::*,
        },
        session::Session,
    },
};
use log::error;
use serde_json::json;


async fn handle_auth_message<'a, T, F, Fut>(
    buffer: &str,
    session: &'a mut Session,
    handler: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(&'a mut Session, T) -> Fut + Send + 'a,
    Fut: Future<Output = Result<(), PPError>> + Send,
{
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

    {
        let session = handler.session.read().await;
        if session.is_authenticated() {
            handler.send_error(method, "You are already authenticated!".into()).await;
            return;
        }
    }

    let res = {
        let mut session = handler.session.write().await;

        match method {
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
        }
    };

    if let Err(err) = res {
        handler.send_error(method, err.to_string().into()).await;
        return;
    }

    if let Some((user_id, session_id)) = handler.session.read().await.get_credentials() {
        let user_id = user_id.get_i32().unwrap();
        {
            let mut connections = handler.sessions.write().await;
            connections.insert(user_id, Arc::clone(&handler.session));
        }


        let data = match method {
            "auth" => json!({ "method": method, "ok": true }),
            _ => json!({ "method": method, "ok": true, "user_id": user_id, "session_id": session_id }),
        };
        handler.send_message(&data).await;
    };
}
