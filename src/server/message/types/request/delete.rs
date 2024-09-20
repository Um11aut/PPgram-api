use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DeleteMessageRequest<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
    pub chat_id: i32,
    pub message_id: i32
}