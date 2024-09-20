use std::borrow::Cow;

use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
/// Main Message type 
pub struct Message<'a> {
    pub message_id: i32,
    pub is_unread: bool,
    pub from_id: i32,
    pub chat_id: i32, 
    pub date: i64,
    pub reply_to: Option<i32>,
    pub content: Option<Cow<'a, str>>,
    pub media_hashes: Vec<Cow<'a, str>>,
    pub media_names: Vec<Cow<'a, str>>
}