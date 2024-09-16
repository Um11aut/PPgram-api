use serde::{Serialize, Deserialize};

use crate::server::message::types::{chat::ChatDetails, request::message::DbMesssage};

#[derive(Serialize, Deserialize)]
pub struct FetchChatsResponseMessage {
    pub ok: bool,
    pub method: String,
    pub chats: Vec<ChatDetails>
}

#[derive(Serialize, Deserialize)]
pub struct FetchUserResponseMessage {
    pub ok: bool,
    pub method: String,
    pub name: String,
    pub user_id: i32,
    pub username: String,
    pub photo: Option<String>,
}

pub type FetchSelfResponseMessage = FetchUserResponseMessage;

#[derive(Serialize, Deserialize)]
pub struct FetchMessagesResponseValue {
    pub ok: bool,
    pub method: String,
    pub messages: Vec<DbMesssage>
}