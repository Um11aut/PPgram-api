use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JoinGroupRequest {
    pub method: String, // join
    pub link: String // +Fnvlksdfjgnv
}