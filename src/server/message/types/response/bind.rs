use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BindResponseMessage {
    pub ok: bool,
    pub method: String,
}