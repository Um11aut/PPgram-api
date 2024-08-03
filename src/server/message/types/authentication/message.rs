use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct RequestAuthMessage {
    pub method: String,
    pub user_id: i32,
    pub password_hash: String,
    pub session_id: String
}


#[derive(Debug, Deserialize, Serialize)]
pub struct RequestLoginMessage {
    pub method: String,
    pub username: String,
    pub password_hash: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestRegisterMessage {
    pub method: String,
    pub name: String,
    pub username: String,
    pub password_hash: String
}