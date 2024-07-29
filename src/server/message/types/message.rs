use serde::{Deserialize, Serialize};

use super::events::media::MediaMessage;
use super::events::text::TextMessage;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommonFields {
    pub method: String,
    pub message_id: u64,
    pub to: i32,
    pub date: u64,
    pub has_reply: bool,
    pub reply_to: u64,
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