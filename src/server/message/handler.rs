use std::borrow::Cow;
use std::slice;
use std::sync::Arc;
use std::{fmt::Write, future::Future, process::Output};

use log::{debug, error, info};
use serde::de::Error as SerdeError;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

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
    #[allow(dead_code)]
    pub(crate) session: Arc<Mutex<Session>>,
    pub(crate) is_first: bool
}

impl RequestMessageHandler {
    pub fn new(writer: Arc<Mutex<OwnedWriteHalf>>, session: Arc<Mutex<Session>>) -> Self {
        RequestMessageHandler {
            builder: None,
            writer,
            session,
            is_first: true
        }
    }

    async fn send_error<T: Into<Cow<'static, str>>>(&self, what: T) {
        PPgramError::send(what, Arc::clone(&self.writer)).await;
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
                            "send_message" | "send_media" => send::handle(self).await,
                            "edit_message" => edit::handle(self).await,
                            "fetch" => fetch::handle(self).await,
                            "check" => check::handle(self).await,
                            _ => self.send_error("Unknown method given!").await
                        }
                    },  
                    None => self.send_error("Failed to get the method from the json message!").await
                }
            },
            Err(err) => {
                self.send_error(err.to_string()).await;
            }
        }
    }

    pub async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        if self.is_first {
            self.builder = Message::parse(buffer);
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
            }
        }
    }
}
