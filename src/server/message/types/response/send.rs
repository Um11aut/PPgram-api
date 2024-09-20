use std::borrow::Cow;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct SendMessageResponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
    pub message_id: i32,
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct UploadMediaResponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
    pub media_hash: Cow<'a, str>
}