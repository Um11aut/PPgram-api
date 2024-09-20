use std::borrow::Cow;

use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct AuthRequest<'a> {
    pub method: Cow<'a, str>,
    pub user_id: i32,
    pub session_id: Cow<'a, str>
}


#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest<'a> {
    pub method: Cow<'a, str>,
    pub username: Cow<'a, str>,
    pub password: Cow<'a, str>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterRequest<'a> {
    pub method: Cow<'a, str>,
    pub name: Cow<'a, str>,
    pub username: Cow<'a, str>,
    pub password: Cow<'a, str>
}