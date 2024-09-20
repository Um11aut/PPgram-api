use std::borrow::Cow;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct AuthResponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterResponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
    pub user_id: i32,
    pub session_id: Cow<'a, str>
}