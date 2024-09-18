use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditSelfMessage {
    pub method: String,
    pub what: String,
    pub name: Option<String>,
    pub username: Option<String>,
    pub photo: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct EditChatMessage {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub message_id: i32,
    pub is_unread: Option<bool>,
    pub reply_to: Option<i32>,
    pub content: Option<String>,
    pub media_hashes: Option<Vec<String>>,
    pub media_names: Option<Vec<String>>
}