use std::borrow::Cow;
use std::ops::Deref;
use std::sync::Arc;

use log::{debug, info};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::{Mutex, RwLock};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::db::internal::error::PPError;
use crate::server::connection::{self, Connection};
use crate::server::server::Sessions;
use crate::server::session::Session;

use super::builder::MessageBuilder;
use super::methods::{auth, bind, check, edit, fetch, send};
use super::types::error::error::PPErrorSender;

pub struct MessageHandler {
    pub builder: Option<MessageBuilder>,
    pub session: Arc<RwLock<Session>>,
    pub sessions: Sessions,
    pub connection: Arc<Connection>,
    is_first: bool,
}

impl MessageHandler {
    pub async fn new(session: Arc<RwLock<Session>>, sessions: Sessions) -> Self {
        let session_locked = session.read().await;
        let current_connection = Arc::clone(&session_locked.connections()[0]);
        drop(session_locked);

        MessageHandler {
            builder: None,
            session: Arc::clone(&session),
            sessions,
            connection: current_connection,
            is_first: true
        }
    }

    pub async fn send_error_str<T: Into<Cow<'static, str>>>(&self, method: &str, what: T) {
        PPErrorSender::send(method, what, &self.connection).await;
    }

    pub async fn send_error(&self, method: &str, err: PPError) {
        err.safe_send(method, &self.connection).await;
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.connection.reader())
    }

    pub async fn send_message<T: ?Sized + Serialize>(&self, message: &T) {
        self.connection.write(&MessageBuilder::build_from(serde_json::to_string(&message).unwrap()).packed()).await;
    }

    pub fn send_msg_to_connection(&self, to: i32, msg: impl Serialize + Send + 'static) {
        tokio::spawn({
            let connections = Arc::clone(&self.sessions);
            async move {
                if let Some(receiver_session) = connections.read().await.get(&to) {
                    let mut target_connection = receiver_session.write().await;
                    
                    target_connection.mpsc_send(msg, 0).await;
                }
            }
        });
    }

    async fn handle_message(&mut self) {
        let message = self.builder.as_ref().unwrap().content();
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
                            "bind" => bind::handle(self, method).await,
                            _ => self.send_error_str(method, "Unknown method given!").await
                        }
                    },  
                    None => self.send_error_str("none", "Failed to get the method from the json message!").await
                }
            },
            Err(err) => {
                self.send_error_str("none", err.to_string()).await;
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

            if let Some(ref message) = self.builder {
                if !message.ready() {
                    return;
                }
            }
        }
        
        let mut do_handle = false;
        if let Some(ref mut message) = self.builder {
            if !message.ready() {
                message.extend(buffer);
            }
            
            if message.ready() {
                do_handle = true;
            }
        }

        if do_handle {
            self.handle_message().await;
    
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
            let connections: Sessions = Arc::clone(&self.sessions);    
            let session = Arc::clone(&self.session);
            let connection = Arc::clone(&self.connection);

            async move {
                let mut connections = connections.write().await;
                let mut session = session.write().await;

                session.remove_connection(connection);
                if let Some((user_id, _)) = session.get_credentials() {
                    if session.connections().is_empty() {
                        connections.remove(&user_id.get_i32().unwrap());
                    }
                }
            }
        });
    }
}