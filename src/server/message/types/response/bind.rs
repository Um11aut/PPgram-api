use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BindReponse {
    pub ok: bool,
    pub method: String,
}