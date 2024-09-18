use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct CheckUsernameRequest {
    pub method: String,
    pub what: String,
    pub data: String
}