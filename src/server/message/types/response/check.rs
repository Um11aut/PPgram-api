use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub method: String,
}