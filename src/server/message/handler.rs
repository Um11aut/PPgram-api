use std::borrow::Cow;
use std::sync::Arc;

use log::debug;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::server::server::Connections;
use crate::server::session::Session;

use super::builder::MessageBuilder;
use super::methods::{auth, check, edit, fetch, send};
use super::types::error::error::PPErrorSender;

pub struct MessageHandler {
    pub(crate) builder: Option<MessageBuilder>,
    pub(crate) writer: Arc<Mutex<OwnedWriteHalf>>,
    pub(crate) session: Arc<Mutex<Session>>,
    is_first: bool,
    pub(crate) connections: Connections
}

impl MessageHandler {
    pub fn new(writer: Arc<Mutex<OwnedWriteHalf>>, session: Arc<Mutex<Session>>, connections: Connections) -> Self {
        MessageHandler {
            builder: None,
            writer,
            session,
            is_first: true,
            connections
        }
    }

    pub async fn send_error<T: Into<Cow<'static, str>>>(&self, method: &str, what: T) {
        PPErrorSender::send(method, what, Arc::clone(&self.writer)).await;
    }

    pub async fn send_message<T: ?Sized + Serialize>(&self, message: &T) {
        self
            .writer
            .lock()
            .await
            .write_all(MessageBuilder::build_from(serde_json::to_string(&message).unwrap()).packed().as_bytes())
            .await
            .unwrap();
    }

    async fn handle_message(&mut self, message: &str) {
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
            self.builder = MessageBuilder::parse(buffer);
            if let Some(builder) = &self.builder {
                debug!("Got the message! \n Message size: {} \n Content: {}", builder.size(), builder.content());
            }
            self.is_first = false;
        }
        
        if let Some(mut message) = self.builder.clone() {
            if !message.ready() {
                message.extend(buffer);
            }
    
            if message.ready() {
                self.handle_message(&message.content()).await;

                message.clear();
                self.builder = None;
                self.is_first = true;
            }
        }
    }
}

impl Drop for MessageHandler {
    fn drop(&mut self) {
        tokio::spawn({
            let connections: Connections = Arc::clone(&self.connections);    
            let session = Arc::clone(&self.session);

            async move {
                let mut connections = connections.write().await;
                let session = session.lock().await;
                if let Some((user_id, _)) = session.get_credentials() {
                    connections.remove(&user_id.get_i32().unwrap());
                }
            }
        });
    }
}