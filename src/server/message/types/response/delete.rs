use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct DeleteMessageResponse {
    pub ok: bool,
    pub method: String,
}