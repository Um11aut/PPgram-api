use serde::{Deserialize, Serialize};

use crate::server::message::types::{
    chat::{ChatDetails, ChatDetailsResponse},
    message::Message,
    user::User,
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
    pub photo: Option<String>,
}

pub type FetchSelfResponse = FetchUserResponse;

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
