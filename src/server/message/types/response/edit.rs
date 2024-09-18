use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct EditMessageResponse {
    pub ok: bool,
    pub method: String,
}