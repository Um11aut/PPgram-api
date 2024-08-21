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

// Define a struct to represent the complete message
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    #[serde(flatten)]
    pub common: CommonFields,
    pub content: MessageContent,
}

pub(crate) type MessageId = i32;

#[derive(Serialize, Deserialize)]
pub struct DbMesssage {
    pub message_id: i32,
    pub is_unread: bool,
    pub from_id: i32,
    pub chat_id: i32, 
    pub date: i64,
    pub reply_to: Option<i32>,
    pub content: Option<String>,
    pub media_datas: Vec<Vec<u8>>,
    pub media_types: Vec<String>,
    pub media_names: Vec<String>
}