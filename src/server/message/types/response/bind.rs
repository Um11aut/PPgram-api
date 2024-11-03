use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BindResponse {
    pub ok: bool,
    pub method: String,
}