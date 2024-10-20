use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommonFields {
    pub method: String,
    // pub message_id: u64,
    pub to: i32,
    // pub date: u64,
    pub has_reply: bool,
    pub reply_to: i32,
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
    pub caption: Option<String>,
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

/// a struct to represent the complete message
#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    #[serde(flatten)]
    pub common: CommonFields,
    pub content: MessageContent,
}

pub type MessageId = i32;
