use std::borrow::Cow;
use std::slice;
use std::sync::Arc;
use std::{fmt::Write, future::Future, process::Output};

use log::{debug, error, info};
use serde::de::Error as SerdeError;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::server::server::Connections;
use crate::server::session::Session;
use crate::server::message::types::{
    authentication::message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage}, message::{MessageContent, RequestMessage},
    error::error::{PPgramError}
};

use super::builder::Message;
use super::handlers::{auth, check, edit, fetch, send};

pub struct RequestMessageHandler {
    pub(crate) builder: Option<Message>,
    pub(crate) writer: Arc<Mutex<OwnedWriteHalf>>,
    pub(crate) session: Arc<Mutex<Session>>,
    pub(crate) is_first: bool,
    pub(crate) connections: Connections
}

impl RequestMessageHandler {
    pub fn new(writer: Arc<Mutex<OwnedWriteHalf>>, session: Arc<Mutex<Session>>, connections: Connections) -> Self {
        RequestMessageHandler {
            builder: None,
            writer,
            session,
            is_first: true,
            connections
        }
    }

    async fn send_error<T: Into<Cow<'static, str>>>(&self, method: &str, what: T) {
        PPgramError::send(method, what, Arc::clone(&self.writer)).await;
    }

    async fn handle_message(&mut self) {
        let builder = self.builder.clone().unwrap();
        let message = builder.content();

        match serde_json::from_str::<Value>(&message) {
            Ok(value) => {
                match value.get("method").and_then(Value::as_str) {
                    Some(method) => {
                        match method {
                            "login" | "auth" | "register" => auth::handle(self, method).await,
                            "send_message" | "send_media" => send::handle(self, method).await,
                            "edit_message" => edit::handle(self, method).await,
                            "fetch" => fetch::handle(self, method).await,
                            "check" => check::handle(self, method).await,
                            _ => self.send_error(method, "Unknown method given!").await
                        }
                    },  
                    None => self.send_error("none", "Failed to get the method from the json message!").await
                }
            },
            Err(err) => {
                self.send_error("none", err.to_string()).await;
            }
        }
    }

    pub async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        if self.is_first {
            self.builder = Message::parse(buffer);
            if let Some(builder) = self.builder.clone() {
                debug!("Got the message! \n Message size: {} \n Content: {}", builder.size(), builder.content());
            }
            self.is_first = false;
        }
        
        if let Some(mut message) = self.builder.clone() {
            if !message.ready() {
                message.extend(buffer);
            }
    
            if message.ready() {
                self.handle_message().await;

                message.clear();
                self.builder = None;
                self.is_first = true;
            }
        }
    }
}

impl Drop for RequestMessageHandler {
    fn drop(&mut self) {
        tokio::spawn({
            let connections: Connections = Arc::clone(&self.connections);    
            let session = Arc::clone(&self.session);

            async move {
                let mut connections = connections.write().await;
                let session = session.lock().await;
                if let Some((user_id, _)) = session.get_credentials() {
                    connections.remove(&user_id);
                }
            }
        });
    }
}