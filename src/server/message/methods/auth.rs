use std::{future::Future, sync::Arc};

use crate::{
    db::internal::error::PPError,
    server::{
        message::{
            handler::MessageHandler,
            types::{request::auth::*, response::auth::{AuthResponse, RegisterResponse}},
        },
        session::Session,
    },
};


async fn handle_auth_message<'a, T, F, Fut>(
    buffer: &str,
    session: &'a mut Session,
    handler: F,
) -> Result<(), PPError>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(&'a mut Session, T) -> Fut + Send + 'a,
    Fut: Future<Output = Result<(), PPError>> + Send,
{
    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => match handler(session, auth_message).await {
            Ok(()) => {}
            Err(err) => return Err(err),
        },
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    let buffer = handler.builder.as_mut().unwrap().content_utf8().unwrap();

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
            "login" =>
                handle_auth_message::<LoginRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    Session::login,
                )
                .await,
            "auth" =>  handle_auth_message::<AuthRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    Session::auth,
                )
                .await,
            "register" => handle_auth_message::<RegisterRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    Session::register,
                )
                .await,
            _ => Err("Invalid method provided!".into()),
        }
    };

    if let Err(err) = res {
        handler.send_error(method, err).await;
        return;
    }

    if let Some((user_id, session_id)) = handler.session.read().await.get_credentials() {
        let user_id = user_id.as_i32().unwrap();
        {
            let mut connections = handler.sessions.write().await;
            connections.insert(user_id, Arc::clone(&handler.session));
        }


        let data = match method {
            "auth" => serde_json::to_value(AuthResponse{ok: true, method: method.into()}).unwrap(),
            _ => serde_json::to_value(RegisterResponse{ ok: true, method: method.into(), user_id, session_id: session_id.into()}).unwrap(),
        };
        handler.send_message(&data).await;
    };
}
