use serde::{Deserialize, Serialize};

use crate::server::message::types::{
    chat::{ChatDetails, ChatDetailsResponse},
    message::Message,
    user::User,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct NewChatEvent {
    pub event: String,
    pub new_chat: ChatDetailsResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewMessageEvent {
    pub event: String,
    pub new_message: Message,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditMessageEvent {
    pub event: String,
    pub new_message: Message,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkAsReadEvent {
    pub event: String, // mark_as_read
    pub chat_id: i32,
    pub message_ids: Vec<i32>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EditSelfEvent {
    pub event: String,
    pub new_profile: User,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteAllMessagesEvent {
    pub event: String,
    pub chat_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteMessagesEvent {
    pub event: String,
    pub chat_id: i32,
    pub message_ids: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteMessageEvent {
    pub event: String, // delete_message
    pub chat_id: i32,
    pub message_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewParticipantEvent {
    pub event: String,
    pub chat_id: i32,
    pub new_user: User,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IsTypingEvent {
    pub event: String, // is_typing
    pub is_typing: bool,
    pub chat_id: i32,
    pub user_id: i32,
}
