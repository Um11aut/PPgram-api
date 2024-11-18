use std::{future::Future, sync::Arc};

use crate::{
    db::{internal::error::{PPError, PPResult}, user::UsersDB},
    server::{
        message::{
            handlers::json_handler::JsonHandler, types::{request::auth::*, response::auth::{AuthResponse, RegisterResponse}}
        },
        session::{AuthComponent, Session},
    },
};


async fn handle_auth_message<'a, T, F, Fut>(
    buffer: &str,
    session: &'a mut Session,
    users_db: UsersDB,
    from_func: F,
) -> Result<(), PPError>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce(UsersDB, T) -> Fut + Send + 'a,
    Fut: Future<Output = PPResult<AuthComponent>> + Send,
{
    match serde_json::from_str::<T>(buffer) {
        Ok(auth_message) => {
            Session::authenticate(session, from_func(users_db, auth_message).await?);
        },
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

pub async fn handle(handler: &mut JsonHandler, method: &str) {
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
                    AuthComponent::from_login,
                )
                .await,
            "auth" =>  handle_auth_message::<AuthRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    users_db,
                    AuthComponent::from_auth,
                )
                .await,
            "register" => handle_auth_message::<RegisterRequest, _, _>(
                    buffer.as_str(),
                    &mut session,
                    users_db,
                    AuthComponent::from_register,
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
