use serde::{Deserialize, Serialize};

use crate::server::message::types::{
    chat::ChatDetailsResponse, message::Message, user::User
};

#[derive(Serialize, Deserialize)]
pub struct FetchChatsResponse {
    pub ok: bool,
    pub method: String,
    pub chats: Vec<ChatDetailsResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct FetchUserResponse {
    pub ok: bool,
    pub method: String,
    pub name: String,
    pub user_id: i32,
    pub username: String,
    pub profile_color: u32,
    pub photo: Option<String>,
}

pub type FetchSelfResponse = FetchUserResponse;

#[derive(Serialize, Deserialize)]
pub struct FetchChatInfoResponse {
    pub ok: bool,
    pub method: String,
    pub photo_count: u32,
    pub video_count: u32,
    pub document_count: u32,
    pub participants: Vec<i32>
}

#[derive(Serialize, Deserialize)]
pub struct FetchMessagesResponse {
    pub ok: bool,
    pub method: String,
    pub messages: Vec<Message>,
}

/// Response on fetching users by search query
#[derive(Deserialize, Serialize)]
pub struct FetchUsersResponse {
    pub ok: bool,
    pub method: String,
    pub users: Vec<User>,
}
