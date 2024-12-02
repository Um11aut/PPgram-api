use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommonFields {
    pub method: String,
    pub to: i32,
    pub reply_to: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageContent {
    pub text: Option<String>,
    pub sha256_hashes: Option<Vec<String>>,
}

/// a struct to represent the complete message
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessageRequest {
    #[serde(flatten)]
    pub common: CommonFields,
    pub content: MessageContent,
}

pub type MessageId = i32;
