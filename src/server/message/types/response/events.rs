use serde::{Serialize, Deserialize};

use crate::server::message::types::{chat::{ChatDetails, ChatId}, message::Message, };

#[derive(Serialize, Deserialize)]
pub struct NewChatEvent {
    pub ok: bool,
    pub event: String,
    pub new_chat: ChatDetails
}

#[derive(Serialize, Deserialize)]
pub struct NewMessageEvent {
    pub ok: bool,
    pub event: String,
    pub new_message: Message
}

#[derive(Serialize, Deserialize)]
pub struct EditMessageEvent {
    pub ok: bool,
    pub event: String,
    pub new_message: Message
}

#[derive(Serialize, Deserialize)]
pub struct DeleteMessageEvent {
    pub ok: bool,
    pub event: String,
    pub chat_id: i32,
    pub message_id: i32
}