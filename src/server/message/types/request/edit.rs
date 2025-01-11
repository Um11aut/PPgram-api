use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditSelfRequest {
    pub method: String,
    pub what: String,
    pub name: Option<String>,
    pub username: Option<String>,
    pub photo: Option<String>,
    pub profile_color: Option<u32>,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MarkAsReadRequest {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub message_ids: Vec<i32>
}

#[derive(Serialize, Deserialize)]
pub struct EditDraftRequest {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub draft: String
}

#[derive(Serialize, Deserialize)]
pub struct EditMessageRequest {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub message_id: i32,
    pub is_unread: Option<bool>,
    pub reply_to: Option<i32>,
    pub content: Option<String>,
    pub sha256_hashes: Option<Vec<String>>,
}
