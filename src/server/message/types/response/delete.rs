use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct DeleteAllMessagesResponse {
    pub ok: bool,
    pub method: String, // delete_all_messages
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct DeleteChatResponse {
    pub ok: bool,
    pub method: String, // delete_chat
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct DeleteMessagesResponse {
    pub ok: bool,
    pub method: String, // delete_messages
    pub chat_id: i32,
    pub message_ids: Vec<i32>
}

