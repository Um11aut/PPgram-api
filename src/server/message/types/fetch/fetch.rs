use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
struct FetchChatsResponseMessage {
    pub method: String,
    pub ok: bool,
    pub chats: Vec<i32> // basically user_ids, because on them the messages are sent
}