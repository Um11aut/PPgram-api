use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct CheckUsernameRequestMessage {
    pub method: String,
    pub ok: bool,
    pub what: String,
    pub data: String
}