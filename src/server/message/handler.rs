use log::{debug, error, info};
use serde::de::Error as _;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::server::session::Session;

use super::{auth_message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage}, message::{MessageContent, RequestMessage}};

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

    async fn send_error_message(&self, socket: &mut tokio::net::TcpStream, error: Option<serde_json::Error>) {
        if let Some(err) = error {
            let data = json!({
                "ok": false,
                "error": err.to_string()
            });
            if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                return;
            }
        } else {
            let data = json!({
                "ok": false,
            });
            if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                return;
            }
        }
    }

    async fn process_json_message(&self, message: &str, socket: &mut tokio::net::TcpStream) {
        let res: Result<RequestMessage, _> = serde_json::from_str(message);
    
        match self.message_size {
            Some(message_size) => {
                if message_size > 1_000_000 {
                    debug!("Processing json message! message_size: {:.1}MB", message_size as f32 / 1_048_576 as f32);
                } else {
                    debug!("Processing json message! message_size: {:.1}KB", message_size as f32 / 1024 as f32);
                }
            },
            None => {},
        }

        if let Err(err) = res {
            error!("{}", err);
            self.send_error_message(socket, Some(err)).await;
        } else {
            let res = res.unwrap();
            let msg_id = res.common.message_id;
            
            let mut data_size: u64 = 0;
            match res.content {
                MessageContent::Media(_) => todo!(),
                MessageContent::Text(message) => data_size = message.text.len() as u64,
            }

            let data = json!({
                "ok": true,
                "data_size": data_size,
                "message_id": msg_id
            });
            if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                return;
            }
        }
    }

    pub async fn handle_segmented_frame(
        &mut self,
        buffer: &[u8],
        socket: &mut tokio::net::TcpStream,
    ) {
        let mut message = std::str::from_utf8(&buffer).unwrap();

        if message.starts_with("size") {
            let re = regex::Regex::new(r"size=(\d+)").unwrap();

            if let Some(caps) = re.captures(message) {
                if let Ok(size) = caps[1].parse::<u32>() {
                    self.message_size = Some(size);
                }
            }

            let pos = message.find("\\n").unwrap();
            message = &message[pos + "\\n".len()..];
        }

        if self.message_size.is_none() {
            let error = serde_json::Error::custom("Message size wasn't provided.");
            self.send_error_message(socket,Some(error)).await;
            return;
        }

        self.temp_buffer.extend_from_slice(message.as_bytes());

        if self.temp_buffer.len() == self.message_size.unwrap() as usize {
            let message = String::from_utf8_lossy(&self.temp_buffer).into_owned();

            self.process_json_message(message.as_str(), socket).await;

            self.temp_buffer.clear();
        } else if self.temp_buffer.len() > self.message_size.unwrap() as usize {
            self.temp_buffer.clear();

            let error = serde_json::Error::custom("Provided size of the message is less than the message itself.");
            self.send_error_message(socket, Some(error)).await;
        }
    }

    pub async fn handle_authentication(
        &self,
        buffer: &[u8],
        socket: &mut tokio::net::TcpStream,
        session: &mut Session
    ) {
        let value: Result<Value, serde_json::Error> = serde_json::from_slice(&buffer);

        if let Ok(value) = value {
            let method = value.get("method").and_then(Value::as_str);

            if let Some(method) = method {
                match method {
                    "login" => {
                        let json_parsed: Result<RequestLoginMessage, _> = serde_json::from_slice(&buffer);

                        if let Ok(json_parsed) = json_parsed {
                            session.login(json_parsed);

                            if session.is_authenticated() {
                                let data = json!({
                                    "ok": true
                                });
                                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                                    return;
                                }
                            } else {
                                self.send_error_message(socket, Some(serde_json::Error::custom("Failed to authenticate with the given data"))).await;
                            }
                        } else {
                            if let Err(err) = json_parsed {
                                self.send_error_message(socket, Some(err)).await;
                            }
                        }
                    }
                    "auth" => {
                        let json_parsed: Result<RequestAuthMessage, _> = serde_json::from_slice(&buffer);

                        if let Ok(json_parsed) = json_parsed {
                            session.auth(json_parsed);

                            if session.is_authenticated() {
                                let data = json!({
                                    "ok": true
                                });
                                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                                    return;
                                }
                            } else {
                                self.send_error_message(socket, Some(serde_json::Error::custom("Failed to authenticate with the given data"))).await;
                            }
                        } else {
                            if let Err(err) = json_parsed {
                                self.send_error_message(socket, Some(err)).await;
                            }
                        }
                    },
                    "register" => {
                        let json_parsed: Result<RequestRegisterMessage, _> = serde_json::from_slice(&buffer);

                        if let Ok(json_parsed) = json_parsed {
                            session.register(json_parsed);

                            if session.is_authenticated() {
                                let data = json!({
                                    "ok": true
                                });
                                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                                    return;
                                }
                            } else {
                                self.send_error_message(socket, Some(serde_json::Error::custom("Failed to authenticate with the given data"))).await;
                            }
                        } else {
                            if let Err(err) = json_parsed {
                                self.send_error_message(socket, Some(err)).await;
                            }
                        }
                    }
                    _ => {}
                }
            } else if let None = method {
                self.send_error_message(socket, Some(serde_json::Error::custom("Didn't get the method value from json!"))).await;
            }

        } else if let Err(err) = value {
            self.send_error_message(socket, Some(err)).await;
        }

        
    }
}
