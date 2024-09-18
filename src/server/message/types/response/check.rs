use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CheckResponseMessage {
    pub ok: bool,
    pub method: String,
}