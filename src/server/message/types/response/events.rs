use serde::{Serialize, Deserialize};

use crate::server::message::types::{chat::ChatDetails, message::Message, };

#[derive(Serialize, Deserialize)]
pub struct NewChatEventResponse {
    pub ok: bool,
    pub event: String,
    pub new_chat: ChatDetails
}

#[derive(Serialize, Deserialize)]
pub struct NewMessageEventResponse {
    pub ok: bool,
    pub event: String,
    pub new_message: Message
}

#[derive(Serialize, Deserialize)]
pub struct EditMessageEventResponse {
    pub ok: bool,
    pub event: String,
    pub new_message: Message
}