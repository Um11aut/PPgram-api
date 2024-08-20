use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct BaseFetchRequestMessage {
    pub method: String,
    pub what: String,
}

#[derive(Deserialize, Serialize)]
pub struct FetchUserRequestMessage {
    pub method: String,
    pub what: String,
    pub username: String
}

#[derive(Deserialize, Serialize, Debug)]
struct FetchChatsResponseMessage {
    pub method: String,
    pub ok: bool,
    pub chats: Vec<i32> // basically user_ids, because on them the messages are sent
}

#[derive(Deserialize, Serialize)]
struct FetchMessagesRequestMessage {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub range: [i32; 2]
}