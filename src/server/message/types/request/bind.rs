use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BindRequestMessage {
    pub method: String,
    pub session_id: String,
    pub user_id: i32
}