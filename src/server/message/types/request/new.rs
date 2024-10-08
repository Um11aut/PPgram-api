use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NewGroupRequest {
    pub method: String,
    pub what: String,
    pub name: String,
    pub avatar_hash: Option<String>,
    pub username: Option<String>,
    pub participants: Vec<i32>
}