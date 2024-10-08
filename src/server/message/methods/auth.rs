use std::{future::Future, sync::Arc};

use crate::{
    db::{internal::error::PPError, user::UsersDB},
    server::{
        message::{
            handler::Handler,
            types::{request::auth::*, response::auth::{AuthResponse, RegisterResponse}},
        },
        session::Session,
    },
};


async fn handle_auth_message<'a, T, F, Fut>(
    buffer: &str,
    session: &'a mut Session,
    users_db: UsersDB,
    handler: F,
) -> Result<(), PPError>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(&'a mut Session, UsersDB, T) -> Fut + Send + 'a,
    Fut: Future<Output = Result<(), PPError>> + Send,
{
    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => match handler(session, users_db, auth_message).await {
            Ok(()) => {}
            Err(err) => return Err(err),
        },
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

pub async fn handle(handler: &mut Handler, method: &str) {
    let buffer = handler.utf8_content_unchecked().clone();

    {
        let session = handler.session.read().await;
        if session.is_authenticated() {
            handler.send_error(method, "You are already authenticated!".into()).await;
            return;
        }
    }

    let res = {
        let mut session = handler.session.write().await;
        let users_db: UsersDB = handler.get_db();

        match method {
            "login" =>
                handle_auth_message::<LoginRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    users_db,
                    Session::login,
                )
                .await,
            "auth" =>  handle_auth_message::<AuthRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    users_db,
                    Session::auth,
                )
                .await,
            "register" => handle_auth_message::<RegisterRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    users_db,
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
            _ => serde_json::to_value(RegisterResponse{ ok: true, method: method.into(), user_id, session_id}).unwrap(),
        };
        handler.send_message(&data).await;
    };
}
