use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UserDetails {
    pub(crate) name: String,
    pub(crate) user_id: i32,
    pub(crate) username: String,
    pub(crate) photo: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct ResponseUserDetails {
    pub(crate) method: String,
    pub(crate) response: UserDetails
}
