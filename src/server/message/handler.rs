use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use log::{debug, info};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::db::internal::error::PPError;
use crate::server::server::Sessions;
use crate::server::session::Session;

use super::builder::MessageBuilder;
use super::methods::{auth, check, edit, fetch, send};
use super::types::error::error::PPErrorSender;

pub struct MessageHandler {
    pub(crate) builder: Option<MessageBuilder>,
    pub(crate) session: Arc<RwLock<Session>>,
    pub(crate) connections: Sessions,
    connection_idx: usize,
    is_first: bool,
}

impl MessageHandler {
    pub fn new(session: Arc<RwLock<Session>>, sessions: Sessions, connection_idx: usize) -> Self {
        MessageHandler {
            builder: None,
            session,
            connections: sessions,
            connection_idx,
            is_first: true
        }
    }

    pub async fn send_err_str<T: Into<Cow<'static, str>>>(&self, method: &str, what: T) {
        let session = self.session.read().await;
        PPErrorSender::send(method, what, &session.connections[self.connection_idx]).await;
    }

    pub async fn send_error(&self, method: &str, err: PPError) {
        let session = self.session.read().await;
        err.safe_send(method, &session.connections[self.connection_idx]).await;
    }

    pub async fn send_message<T: ?Sized + Serialize>(&self, message: &T) {
        let session = self.session.read().await;
        session.connections[self.connection_idx].write(&MessageBuilder::build_from(serde_json::to_string(&message).unwrap()).packed()).await;
    }

    pub fn send_msg_to_connection(&self, to: i32, msg: impl Serialize + Send + 'static) {
        tokio::spawn({
            let connections = Arc::clone(&self.connections);
            async move {
                if let Some(receiver_session) = connections.read().await.get(&to) {
                    let mut target_connection = receiver_session.write().await;
                    
                    target_connection.mpsc_send(msg, 0).await;
                }
            }
        });
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
                            _ => self.send_err_str(method, "Unknown method given!").await
                        }
                    },  
                    None => self.send_err_str("none", "Failed to get the method from the json message!").await
                }
            },
            Err(err) => {
                self.send_err_str("none", err.to_string()).await;
            }
        }
    }

    pub async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        if self.is_first {
            self.builder = MessageBuilder::parse(buffer);
            if let Some(builder) = &self.builder {
                debug!("Got the message! \n Message size: {}", builder.size());
            }
            self.is_first = false;
            if !self.builder.as_ref().unwrap().ready() {
                return;
            }
        }
        
        let mut content = String::new();
        if let Some(ref mut message) = self.builder {
            if !message.ready() {
                message.extend(buffer);
            }
            
            if message.ready() {
                content = message.content().clone();
            }
        }

        if !content.is_empty() {
            self.handle_message(&content).await;
    
            if let Some(ref mut message) = self.builder {
                message.clear();
            }
            self.builder = None;
            self.is_first = true;
        }
    }
}

impl Drop for MessageHandler {
    fn drop(&mut self) {
        tokio::spawn({
            let connections: Sessions = Arc::clone(&self.connections);    
            let session = Arc::clone(&self.session);

            async move {
                let mut connections = connections.write().await;
                let session = session.read().await;
                if let Some((user_id, _)) = session.get_credentials() {
                    connections.remove(&user_id.get_i32().unwrap());
                }
            }
        });
    }
}