use std::sync::Arc;
use base64::prelude::*;

use log::{debug, info};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::{Mutex, RwLock};

use crate::db::bucket::{DatabaseBucket, DatabaseBuilder};
use crate::db::db::Database;
use crate::db::internal::error::PPError;
use crate::fs::media::add_media;
use crate::server::connection::TCPConnection;
use crate::server::message::builder::MessageBuilder;
use crate::server::message::methods::{auth, bind, check, edit, fetch, join, new, send};
use crate::server::message::Handler;
use crate::server::server::Sessions;
use crate::server::session::Session;

pub type SesssionArcRwLock = Arc<RwLock<Session>>;

const MAX_MSG_SIZE: u32 = 100_000_000 /* 100Mb */;

/// TCP Message handler struct that has everything it needs to have to be able
/// to handle any JSON message type
pub struct TCPHandler {
    builder: Option<MessageBuilder>,
    is_first: bool,
    pub session: SesssionArcRwLock,
    pub sessions: Sessions,
    // Output TCP connection on which all the responses/messages are sent
    pub output_connection: Arc<TCPConnection>,
    bucket: DatabaseBucket
}

#[async_trait::async_trait]
impl Handler for TCPHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        if self.is_first {
            self.builder = MessageBuilder::parse(buffer);
            if let Some(builder) = &self.builder {
                if builder.size() == 0 {
                    self.send_error("none", "Message size cannot be 0!".into()).await;
                    self.builder = None;
                    self.is_first = true;
                    return;
                }

                // if message size exceeds the maximum size do not handle it.
                if builder.size() > MAX_MSG_SIZE {
                    self.send_error("none", "Message size cannot be 0!".into()).await;
                    self.builder = None;
                    self.is_first = true;
                    return;
                }

                if builder.size() < 1024 {
                    debug!("Got the message! \n Message size: {} \n Message Content: {}", builder.size(), String::from_utf8_lossy(builder.content_bytes()));
                } else {
                    debug!("Got the message! \n Message size: {}", builder.size());
                }
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
            if self.builder.is_some() {
                self.try_handle_json_message().await;
            }
    
            if let Some(ref mut message) = self.builder {
                message.clear();
            }
            self.builder = None;
            self.is_first = true;
        }
    }
}

impl TCPHandler {
    pub async fn new(session: Arc<RwLock<Session>>, sessions: Sessions, bucket: DatabaseBucket) -> Self {
        let output_connection = {
            let session_locked = session.read().await;
            // Assume the first connection is the output connection
            Arc::clone(&session_locked.connections()[0])
        };

        TCPHandler {
            builder: None,
            session: Arc::clone(&session),
            sessions,
            output_connection,
            is_first: true,
            bucket 
        }
    }

    /// Later, we need to retreive the content of the `self.builder`
    pub fn utf8_content_unchecked(&mut self) -> &String {
        self.builder.as_mut().unwrap().content_utf8().unwrap()
    }

    /// Sends standartized json error:
    /// 
    /// ok: false,
    /// method as `str`,
    /// err as `str`
    pub async fn send_error(&self, method: &str, err: PPError) {
        err.safe_send(method, &self.output_connection).await;
    }

    /// Sending message with tokio::spawn. 
    /// Necessary for large media, so the read operation won't be stopped
    pub fn send_raw_detached(&self, data: Arc<[u8]>) {
        tokio::spawn({
            let connection = Arc::clone(&self.output_connection);
            let data = Arc::clone(&data);
            info!("Sending media back: {}", data.len());
            async move {
                connection.write(&MessageBuilder::build_from_vec(&data).packed()).await
            }
        });
    }

    /// Instead of json, sends raw buffer directly to the connection
    pub async fn send_raw(&self, data: &[u8]) {
        self.output_connection.write(&MessageBuilder::build_from_vec(&data).packed()).await;
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.output_connection.reader())
    }

    pub async fn send_message<T: ?Sized + Serialize>(&self, message: &T) {
        self.output_connection.write(&MessageBuilder::build_from_str(serde_json::to_string(&message).unwrap()).packed()).await;
    }

    /// Sends message to other connection(e.g. new chat, new message, or any other event that must be handled in realtime)
    /// 
    /// If user isn't connected to the server, nothing happens
    pub fn send_msg_to_connection_detached(&self, to: i32, msg: impl Serialize + Send + 'static) {
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

    // Function to get any database by just passing the type
    pub fn get_db<T: From<DatabaseBuilder>>(&self) -> T {
        DatabaseBuilder::from(self.bucket.clone()).into()
    }

    async fn try_handle_json_message(&mut self) {
        let message = self.builder.as_mut().unwrap().content_utf8();
        if message.is_none() {
            self.send_error("none", "Invalid utf8 sequence!".into()).await;
            return
        }
        let message = message.unwrap();

        if !message.ends_with('}') {
            self.send_error("none", "Invalid json string sequence!".into()).await;
            return
        }

        match serde_json::from_str::<Value>(&message) {
            Ok(value) => {
                match value.get("method").and_then(Value::as_str) {
                    Some(method) => {
                        match method {
                            "login" | "auth" | "register" => auth::handle(self, method).await,
                            "send_message" => send::handle(self, method).await,
                            "edit" | "delete" => edit::handle(self, method).await,
                            "fetch" => fetch::handle(self, method).await,
                            "check" => check::handle(self, method).await,
                            "bind" => bind::handle(self, method).await,
                            "new" => new::handle(self, method).await,
                            "join" => join::handle(self, method).await,
                            _ => self.send_error(method, "Unknown method given!".into()).await
                        }
                    },  
                    None => self.send_error("none", "Failed to get the method from the json message!".into()).await
                }
            },
            Err(err) => {
                self.send_error("none", err.to_string().into()).await;
            }
        }
    }

    // async fn try_handle_media_message(&mut self) {
    //     let message = self.builder.as_ref().unwrap().content_bytes();
    //     match add_media(message).await {
    //         Ok(sha256_hash) => {
    //             self.send_message(&UploadMediaResponse{
    //                 ok: true,
    //                 method: "send_media".into(),
    //                 media_hash: sha256_hash
    //             }).await;
    //         },
    //         Err(err) => self.send_error("send_media", err).await,
    //     }
    // }
}

impl Drop for TCPHandler {
    fn drop(&mut self) {
        self.bucket.decrement_rc(); 

        tokio::spawn({
            let connections: Sessions = Arc::clone(&self.sessions);    
            let session = Arc::clone(&self.session);
            let connection = Arc::clone(&self.output_connection);

            async move {
                let mut connections = connections.write().await;
                let mut session = session.write().await;

                session.remove_connection(connection);
                if let Some((user_id, _)) = session.get_credentials() {
                    if session.connections().is_empty() {
                        connections.remove(&user_id.as_i32().unwrap());
                    }
                }
            }
        });
    }
}