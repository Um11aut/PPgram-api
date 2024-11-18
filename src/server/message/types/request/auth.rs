use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthRequest {
    pub method: String,
    pub user_id: i32,
    pub session_id: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    pub method: String,
    pub username: String,
    pub password: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterRequest {
    pub method: String,
    pub name: String,
    pub username: String,
    pub password: String
}
