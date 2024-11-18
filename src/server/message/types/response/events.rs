use serde::{Serialize, Deserialize};

use crate::server::message::types::{chat::ChatDetails, message::Message, user::User, };

#[derive(Serialize, Deserialize)]
pub struct NewChatEvent {
    pub event: String,
    pub new_chat: ChatDetails
}

#[derive(Serialize, Deserialize)]
pub struct NewMessageEvent {
    pub event: String,
    pub new_message: Message
}

#[derive(Serialize, Deserialize)]
pub struct EditMessageEvent {
    pub event: String,
    pub new_message: Message
}

#[derive(Serialize, Deserialize)]
pub struct EditSelfEvent {
    pub event: String,
    pub new_profile: User
}


#[derive(Serialize, Deserialize)]
pub struct DeleteMessageEvent {
    pub event: String,
    pub chat_id: i32,
    pub message_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct NewParticipantEvent {
    pub event: String,
    pub chat_id: i32,
    pub new_user: User
}
