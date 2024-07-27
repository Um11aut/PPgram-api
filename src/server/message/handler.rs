use std::{fmt::Write, future::Future, process::Output};

use log::{debug, error, info};
use serde::de::Error as SerdeError;
use serde_json::{json, Value};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::server::session::Session;
use crate::server::message::types::{
    authentication::message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage}, message::{MessageContent, RequestMessage}
};

use super::builder::Message;

pub(crate) struct RequestMessageHandler {
    temp_buffer: Vec<u8>,
    message_size: Option<u32>,
}

impl RequestMessageHandler {
    pub fn new() -> Self {
        RequestMessageHandler {
            temp_buffer: Vec::new(),
            message_size: None,
        }
    }

    async fn send_error_message(&self, socket: &mut OwnedWriteHalf, error: Option<serde_json::Error>) {
        let data = if let Some(err) = error {
            json!({ "ok": false, "error": err.to_string() })
        } else {
            json!({ "ok": false })
        };

        let message_builder = Message::build_from(serde_json::to_string(&data).unwrap());
        if socket.write_all(message_builder.packed().as_bytes()).await.is_err() {
            error!("Failed to send error message");
        }
    }

    async fn process_json_message(&self, message: &str, socket: &mut OwnedWriteHalf) {
        match serde_json::from_str::<RequestMessage>(message) {
            Ok(res) => {
                let msg_id = res.common.message_id;
                let data_size = match res.content {
                    MessageContent::Media(_) => todo!(),
                    MessageContent::Text(ref message) => message.text.len() as u64,
                };

                let data = json!({
                    "ok": true,
                    "data_size": data_size,
                    "message_id": msg_id
                });

                let message_builder = Message::build_from(serde_json::to_string(&data).unwrap());
                if socket.write_all(message_builder.packed().as_bytes()).await.is_err() {
                    error!("Failed to send message response");
                }
            }
            Err(err) => {
                error!("Failed to process JSON message: {}", err);
                self.send_error_message(socket, Some(err)).await;
            }
        }
    }

    pub async fn handle_segmented_frame(&mut self, buffer: &[u8], socket: &mut OwnedWriteHalf) {
        let message_builder = Message::parse(&buffer);

        match message_builder {
            Some(message) => {
                if message.has_header() {
                    self.message_size = Some(message.size());
                }
                
                let message = message.content();
                let slice: &[u8] = message.as_bytes();
                self.temp_buffer.extend_from_slice(slice);
            
                if let Some(message_size) = self.message_size {
                    if self.temp_buffer.len() >= message_size as usize {
                        let message = String::from_utf8_lossy(&self.temp_buffer).into_owned();
                        self.process_json_message(&message, socket).await;
                        self.temp_buffer.clear();
                    }
                }
            },
            None => {
                self.send_error_message(socket, Some(SerdeError::custom("Invalid UTF-8 content sequence!"))).await;
            },
        }
    }

    async fn handle_auth_message<'a, T, F, Fut>(&mut self, buffer: &str, socket: &mut OwnedWriteHalf, session: &'a mut Session, handler: F)
    where
        T: serde::de::DeserializeOwned,
        F: FnOnce(&'a mut Session, T) -> Fut,
        Fut: Future<Output = bool>,
    {
        match serde_json::from_str::<T>(buffer) {
            Ok(auth_message) => {
                let data = if handler(session, auth_message).await {
                    json!({ "ok": true })
                } else {
                    json!({ "ok": false, "error": "Failed to authenticate with the given data" })
                };

                let message_builder = Message::build_from(serde_json::to_string(&data).unwrap());
                if socket.write_all(message_builder.packed().as_bytes()).await.is_err() {
                    error!("Failed to send authentication response");
                }
            }
            Err(err) => {
                self.send_error_message(socket, Some(err)).await;
            }
        }
    }


    pub async fn handle_authentication(&mut self, buffer: &[u8], socket: &mut OwnedWriteHalf, session: &mut Session) {
        let message_builder = Message::parse(buffer);

        if let None = message_builder {
            self.send_error_message(socket, Some(SerdeError::custom("Invalid UTF-8 content sequence!"))).await;
            return;
        }

        let message = message_builder.unwrap();
        let buffer = message.content();

        if message.has_header() {
            self.message_size = Some(message.size())
        }
        self.temp_buffer.extend_from_slice(buffer.as_bytes());

        if let Some(message_size) = self.message_size {
            if message_size >= self.temp_buffer.len() as u32 {
                match serde_json::from_str::<Value>(&buffer) {
                    Ok(value) => {
                        if let Some(method) = value.get("method").and_then(Value::as_str) {
                            match method {
                                "login" => self.handle_auth_message::<RequestLoginMessage, _, _>(&buffer, socket, session, Session::login).await,
                                "auth" => self.handle_auth_message::<RequestAuthMessage, _, _>(&buffer, socket, session, Session::auth).await,
                                "register" => self.handle_auth_message::<RequestRegisterMessage, _, _>(&buffer, socket, session, Session::register).await,
                                _ => self.send_error_message(socket, Some(SerdeError::custom("Unknown method"))).await,
                            }
                        } else {
                            self.send_error_message(socket, Some(SerdeError::custom("Didn't get the method value from json!"))).await;
                        }
                    }
                    Err(err) => self.send_error_message(socket, Some(err)).await,
                }
            
                self.temp_buffer.clear();
            }
        }

    }
}
