use std::borrow::Cow;

use serde::{Serialize, Deserialize};

use crate::server::message::types::{chat::ChatDetails, message::Message};

#[derive(Serialize, Deserialize)]
pub struct FetchChatsResponse {
    pub ok: bool,
    pub method: String,
    pub chats: Vec<ChatDetails>
}

#[derive(Serialize, Deserialize)]
pub struct FetchUserResponse {
    pub ok: bool,
    pub method: String,
    pub name: String,
    pub user_id: i32,
    pub username: String,
    pub photo: Option<String>,
}

pub type FetchSelfResponseMessage = FetchUserResponse;

#[derive(Serialize, Deserialize)]
pub struct FetchMessagesResponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
    pub messages: Vec<Message<'a>>
}