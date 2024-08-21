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

#[derive(Deserialize, Serialize)]
pub struct FetchMessagesRequestMessage {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub range: [i32; 2]
}