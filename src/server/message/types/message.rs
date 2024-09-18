use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
/// Main Message type 
pub struct Message {
    pub message_id: i32,
    pub is_unread: bool,
    pub from_id: i32,
    pub chat_id: i32, 
    pub date: i64,
    pub reply_to: Option<i32>,
    pub content: Option<String>,
    pub media_hashes: Vec<String>,
    pub media_names: Vec<String>
}