use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct AuthResponse {
    pub ok: bool,
    pub method: String,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterResponse {
    pub ok: bool,
    pub method: String,
    pub user_id: i32,
    pub session_id: String
}