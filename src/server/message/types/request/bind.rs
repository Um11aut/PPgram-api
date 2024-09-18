use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BindRequest {
    pub method: String,
    pub session_id: String,
    pub user_id: i32
}