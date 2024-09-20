use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditSelfRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub name: Option<Cow<'a, str>>,
    pub username: Option<Cow<'a, str>>,
    pub photo: Option<Cow<'a, str>>,
    pub password: Option<Cow<'a, str>>,
}

#[derive(Serialize, Deserialize)]
pub struct EditMessageRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub chat_id: i32,
    pub message_id: i32,
    pub is_unread: Option<bool>,
    pub reply_to: Option<i32>,
    pub content: Option<Cow<'a, str>>,
    pub media_hashes: Option<Vec<Cow<'a, str>>>,
    pub media_names: Option<Vec<Cow<'a, str>>>
}