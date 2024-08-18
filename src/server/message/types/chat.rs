use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ChatDetails {
    pub(crate) name: String,
    pub(crate) photo: Vec<u8>,
    pub(crate) username: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ResponseChatsDetails {
    pub(crate) method: String,
    pub(crate) response: Vec<ChatDetails>,
}

#[derive(Debug)]
pub(crate) struct Chat {
    pub(crate) chat_id: i32,
    pub(crate) is_group: bool,
    pub(crate) participants: Vec<i32>
}