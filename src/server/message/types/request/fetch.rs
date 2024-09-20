use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct BaseFetchRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
}

#[derive(Deserialize, Serialize)]
pub struct FetchUserRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub username: Option<Cow<'a, str>>,
    pub user_id: Option<i32>
}

#[derive(Deserialize, Serialize)]
pub struct FetchMessagesRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub chat_id: i32,
    pub range: [i32; 2]
}

#[derive(Deserialize, Serialize)]
pub struct FetchMediaRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub media_hash: Cow<'a, str>
}