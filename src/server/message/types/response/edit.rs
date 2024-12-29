use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditMessageResponse {
    pub ok: bool,
    pub method: String,
}

#[derive(Serialize, Deserialize)]
pub struct EditDraftResponse {
    pub ok: bool,
    pub method: String,
}
