use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DeleteAllMessagesRequest {
    pub method: String, // delete
    pub what: String, // all_messages
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct DeleteChatRequest {
    pub method: String, // delete
    pub what: String, // chat
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct DeleteMessagesRequest {
    pub method: String, // delete
    pub what: String, // messages
    pub chat_id: i32,
    pub message_ids: Vec<i32>
}

