use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditMessageResponse {
    pub ok: bool,
    pub method: String,
}

#[derive(Serialize, Deserialize)]
pub struct EditIsUnreadResponse {
    pub ok: bool,
    pub method: String,
    pub chat_id: i32
}

#[derive(Serialize, Deserialize)]
pub struct EditDraftResponse {
    pub ok: bool,
    pub method: String,
}
