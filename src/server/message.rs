use log::error;
use serde::{Deserialize, Serialize};
use serde_json::Result;

use serde_json::json;
use tokio::io::AsyncWriteExt;
use std::collections::VecDeque;

const PACKET_SIZE: u32 = 1024;
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
    parts: VecDeque<u32>,
    is_header: bool,
    message_size: Option<u32>,
    inital_msg_len: u32
}

impl RequestMessageHandler {
    pub fn new() -> Self {
        RequestMessageHandler {
            temp_buffer: Vec::new(),
            parts: VecDeque::new(),
            is_header: false,
            message_size: None,
            inital_msg_len: 0
        }
    }

    pub async fn handle_segmented_frame(
        &mut self,
        buffer: &[u8],
        socket: &mut tokio::net::TcpStream,
    ) {
        let mut message = std::str::from_utf8(&buffer).unwrap();

        let mut pos: Option<_> = None;
        let mut rest: Option<&str> = None;
        self.inital_msg_len = message.len() as u32;

        if message.starts_with("size") {
            let re = regex::Regex::new(r"size=(\d+)").unwrap();

            if let Some(caps) = re.captures(message) {
                if let Ok(size) = caps[1].parse::<u32>() {
                    self.message_size = Some(size);
                }
            }

            self.is_header = true;
            pos = Some(message.find("\\n").unwrap());
            rest = Some(&message[0..pos.unwrap()+2]);
            message = &message[pos.unwrap() + 2..];

            if let Some(message_size) = self.message_size {
                self.temp_buffer = Vec::with_capacity(1024);
                
                let mut remaining = message_size + rest.unwrap().len() as u32;
                
                while remaining > 0 {
                    let chunk = if remaining > PACKET_SIZE {
                        PACKET_SIZE
                    } else {
                        remaining
                    };
                    self.parts.push_back(chunk);
                    remaining -= chunk;
                }
            }
        }

        if self.parts.is_empty() && self.is_header {
            return;
        }

        if *self.parts.front().unwrap() != self.inital_msg_len as u32 {
            error!("Invalid parts: {} != {}, is_header: {}", self.inital_msg_len, *self.parts.front().unwrap(), self.is_header);
            return;
        }

        self.temp_buffer.extend_from_slice(message.as_bytes());

        if self.temp_buffer.len() == self.message_size.unwrap() as usize {
            let message = String::from_utf8_lossy(&self.temp_buffer).into_owned();
            let res: Result<RequestMessage, > = serde_json::from_str(message.as_str());

            if let Err(err) = res {
                error!("{}", err);
                let data = json!({
                    "ok": false
                });
                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                    return;
                }
            } else {
                let data = json!({
                    "ok": true
                });
                if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                    return;
                }
            }

            self.temp_buffer.clear();
        }

        if self.is_header {
            self.is_header = false;
        }

        self.parts.pop_front();
    }
}
