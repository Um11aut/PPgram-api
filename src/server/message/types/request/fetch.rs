use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct BaseFetchRequest {
    pub method: String,
    pub what: String,
}

#[derive(Deserialize, Serialize)]
pub struct FetchUserRequest {
    pub method: String,
    pub what: String,
    pub username: Option<String>,
    pub user_id: Option<i32>
}

#[derive(Deserialize, Serialize)]
pub struct FetchMessagesRequest {
    pub method: String,
    pub what: String,
    pub chat_id: i32,
    pub range: [i32; 2]
}

#[derive(Deserialize, Serialize)]
pub struct FetchMediaRequest {
    pub method: String,
    pub what: String,
    pub media_hash: String
}

/// Fetches users using indirect search by part of the username
#[derive(Deserialize, Serialize)]
pub struct FetchUsersRequest {
    pub method: String, // fetch
    pub what: String, // users
    pub query: String // @pep or pep or whatever
}
