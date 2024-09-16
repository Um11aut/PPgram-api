use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub ok: bool,
    pub method: String,
    pub message_id: i32,
    pub chat_id: i32
}