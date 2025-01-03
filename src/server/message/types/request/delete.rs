use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DeleteMessageRequest {
    pub method: String,
    pub chat_id: i32,
    pub message_id: i32
}
