use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Result;

use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use tokio::net::TcpSocket;
use std::collections::VecDeque;

const PACKET_SIZE: u32 = 65000;
#[derive(Debug, Serialize, Deserialize)]
pub struct CommonFields {
    pub method: String,
    pub message_id: u64,
    pub to: u64,
    pub date: u64,
    pub has_reply: bool,
    pub reply_to: u64,
}

// Define a struct to represent a single media item
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaItem {
    pub file_name: String,
    pub format: String,
    pub data: String,
}

// Define a struct to represent media messages with multiple media items
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaMessage {
    pub media: Vec<MediaItem>,
    pub has_caption: bool,
    pub caption: String,
}

// Define a struct to represent text messages
#[derive(Debug, Serialize, Deserialize)]
pub struct TextMessage {
    pub text: String,
}

// Define an enum to represent different types of messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Media(MediaMessage),
    Text(TextMessage),
}

// Define a struct to represent the complete message
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RequestMessage {
    #[serde(flatten)]
    pub common: CommonFields,
    pub content: MessageContent,
}

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

    async fn process_json_message(&self, message: &str, socket: &mut tokio::net::TcpStream) {
        let res: Result<RequestMessage, > = serde_json::from_str(message);
            if let Err(err) = res {
                error!("{}", err);
                let data = json!({
                    "ok": false
                });
                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                    return;
                }
            } else {
                let res = res.unwrap();
                let msg_id = res.common.message_id;
                
                let mut data_size: u64 = 0;
                match res.content {
                    MessageContent::Media(media) => todo!(),
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
            message = &message[pos + 2..];
        }

        self.temp_buffer.extend_from_slice(message.as_bytes());

        if self.temp_buffer.len() == self.message_size.unwrap() as usize {
            let message = String::from_utf8_lossy(&self.temp_buffer).into_owned();

            self.process_json_message(message.as_str(), socket).await;

            self.temp_buffer.clear();
        }
    }
}
