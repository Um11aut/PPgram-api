use std::sync::Arc;

use log::debug;

use crate::server::message::{handlers::json_handler::TCPHandler, types::{request::bind::BindRequest, response::bind::BindResponse}};

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    let content = handler.utf8_content_unchecked();
    match serde_json::from_str::<BindRequest>(&content) {
        Ok(message) => {
            if message.method != "bind" {
                handler.send_error(method, "method must be 'bind'.".into()).await;
                return;
            }

            let connections = handler.sessions.read().await;
            if let Some(bind_session_arc) = connections.get(&message.user_id) {
                let mut bind_session = bind_session_arc.write().await;
                if bind_session.session_id().unwrap() == &message.session_id {
                    {
                        let mut self_session = handler.session.write().await;
                        self_session.remove_connection(Arc::clone(&handler.output_connection));
                        bind_session.add_connection(Arc::clone(&handler.output_connection));
                    }
                    handler.session = Arc::clone(&bind_session_arc);
                    debug!("Binding to session: {}", bind_session.session_id().unwrap());
                    drop(bind_session);
                    handler.send_message(&BindResponse{
                        ok: true,
                        method: method.into()
                    }).await;
                } else {
                    handler.send_error(method, "User with given `session_id` isn't connected to the server".into()).await;
                }
            } else {
                handler.send_error(method, "User with given `user_id` isn't connected to the server".into()).await;
            }
        },
        Err(err) => {
            handler.send_error(method, err.to_string().into()).await;
        }
    }
}