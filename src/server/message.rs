use serde::{Deserialize, Serialize};
use serde_json::Result;

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

impl RequestMessage {
    
}