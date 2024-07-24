use std::fmt::Write;

use log::{debug, error, info};
use serde::de::Error as SerdeError;
use serde_json::{json, Value};
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf};

use crate::server::session::Session;
use super::{
    auth_message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage},
    message::{MessageContent, RequestMessage},
};

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

        if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
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

                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                    error!("Failed to send message response");
                }
            }
            Err(err) => {
                error!("Failed to process JSON message: {}", err);
                self.send_error_message(socket, Some(err)).await;
            }
        }
    }

    async fn parse_message_size<'a>(&mut self, message: &'a str) -> Result<&'a str, serde_json::Error> {
        if message.starts_with("size") {
            let re = regex::Regex::new(r"size=(\d+)").map_err(|e| SerdeError::custom(e.to_string()))?;
            if let Some(caps) = re.captures(message) {
                if let Ok(size) = caps[1].parse::<u32>() {
                    self.message_size = Some(size);
                }
            }

            if let Some(pos) = message.find("\\n") {
                return Ok(&message[pos + "\\n".len()..]);
            }
        }

        Err(SerdeError::custom("Message size wasn't provided."))
    }

    pub async fn handle_segmented_frame(&mut self, buffer: &[u8], socket: &mut OwnedWriteHalf) {
        match std::str::from_utf8(&buffer) {
            Ok(mut message) => {
                if self.message_size.is_none() {
                    match self.parse_message_size(message).await {
                        Ok(parsed_message) => message = parsed_message,
                        Err(err) => {
                            self.send_error_message(socket, Some(err)).await;
                            return;
                        }
                    }
                }

                self.temp_buffer.extend_from_slice(message.as_bytes());

                if let Some(message_size) = self.message_size {
                    if self.temp_buffer.len() == message_size as usize {
                        let message = String::from_utf8_lossy(&self.temp_buffer).into_owned();
                        self.process_json_message(&message, socket).await;
                        self.temp_buffer.clear();
                    } else if self.temp_buffer.len() > message_size as usize {
                        self.temp_buffer.clear();
                        self.send_error_message(socket, Some(SerdeError::custom("Provided size of the message is less than the message itself."))).await;
                    }
                }
            }
            Err(err) => {
                self.send_error_message(socket, Some(SerdeError::custom(format!("Invalid UTF-8 sequence: {}", err)))).await;
            }
        }
    }

    async fn handle_auth_message<T>(&self, buffer: &[u8], socket: &mut OwnedWriteHalf, session: &mut Session, handler: fn(&mut Session, T) -> ())
    where
        T: serde::de::DeserializeOwned,
    {
        match serde_json::from_slice::<T>(buffer) {
            Ok(auth_message) => {
                handler(session, auth_message);
                if session.is_authenticated() {
                    let data = json!({ "ok": true });
                    if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                        error!("Failed to send authentication response");
                    }
                } else {
                    self.send_error_message(socket, Some(SerdeError::custom("Failed to authenticate with the given data"))).await;
                }
            }
            Err(err) => self.send_error_message(socket, Some(err)).await,
        }
    }

    pub async fn handle_authentication(&self, buffer: &[u8], socket: &mut OwnedWriteHalf, session: &mut Session) {
        match serde_json::from_slice::<Value>(buffer) {
            Ok(value) => {
                if let Some(method) = value.get("method").and_then(Value::as_str) {
                    match method {
                        "login" => self.handle_auth_message::<RequestLoginMessage>(buffer, socket, session, Session::login).await,
                        "auth" => self.handle_auth_message::<RequestAuthMessage>(buffer, socket, session, Session::auth).await,
                        "register" => self.handle_auth_message::<RequestRegisterMessage>(buffer, socket, session, Session::register).await,
                        _ => self.send_error_message(socket, Some(SerdeError::custom("Unknown method"))).await,
                    }
                } else {
                    self.send_error_message(socket, Some(SerdeError::custom("Didn't get the method value from json!"))).await;
                }
            }
            Err(err) => self.send_error_message(socket, Some(err)).await,
        }
    }
}
