use std::sync::Arc;

use log::{debug, info};
use serde_json::json;

use crate::server::message::{handler::MessageHandler, types::request::bind::BindRequestMessage};

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    match serde_json::from_str::<BindRequestMessage>(&handler.builder.as_ref().unwrap().content()) {
        Ok(message) => {
            if message.method != "bind" {
                handler.send_err_str(method, "method must be 'bind'.").await;
                return;
            }

            let connections = handler.connections.read().await;
            if let Some(bind_session_arc) = connections.get(&message.user_id) {
                let mut bind_session = bind_session_arc.write().await;
                if bind_session.session_id().unwrap() == &message.session_id {
                    let connection_idx = {
                        {
                            let mut self_session = handler.session.write().await;
                            bind_session.connections.push(self_session.connections.remove(0));
                        }
                        bind_session.connections.len() - 1
                    };
                    handler.connection_idx = connection_idx;
                    handler.session = Arc::clone(&bind_session_arc);
                    debug!("Binding to session: {}", bind_session.session_id().unwrap());
                    drop(bind_session);
                    handler.send_message(&json!({
                        "method": method,
                        "ok": true
                    })).await;
                } else {
                    handler.send_err_str(method, "User with given `session_id` isn't connected to the server").await;
                }
            } else {
                handler.send_err_str(method, "User with given `user_id` isn't connected to the server").await;
            }
        },
        Err(err) => {
            handler.send_err_str(method, err.to_string()).await;
        }
    }
}