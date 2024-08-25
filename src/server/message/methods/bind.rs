use std::sync::Arc;

use log::{debug, info};
use serde_json::json;

use crate::server::message::{handler::MessageHandler, types::request::bind::BindRequestMessage};

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    match serde_json::from_str::<BindRequestMessage>(&handler.builder.as_ref().unwrap().content()) {
        Ok(message) => {
            if message.method != "bind" {
                handler.send_error_str(method, "method must be 'bind'.").await;
                return;
            }

            let connections = handler.sessions.read().await;
            if let Some(bind_session_arc) = connections.get(&message.user_id) {
                let mut bind_session = bind_session_arc.write().await;
                if bind_session.session_id().unwrap() == &message.session_id {
                    {
                        let mut self_session = handler.session.write().await;
                        self_session.remove_connection(Arc::clone(&handler.connection));
                        bind_session.add_connection(Arc::clone(&handler.connection));
                    }
                    handler.session = Arc::clone(&bind_session_arc);
                    debug!("Binding to session: {}", bind_session.session_id().unwrap());
                    drop(bind_session);
                    handler.send_message(&json!({
                        "method": method,
                        "ok": true
                    })).await;
                } else {
                    handler.send_error_str(method, "User with given `session_id` isn't connected to the server").await;
                }
            } else {
                handler.send_error_str(method, "User with given `user_id` isn't connected to the server").await;
            }
        },
        Err(err) => {
            handler.send_error_str(method, err.to_string()).await;
        }
    }
}