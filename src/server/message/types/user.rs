use std::sync::Arc;

pub(crate) struct UserInfo {
    pub(crate) name: String,
    pub(crate) user_id: i32,
    pub(crate) username: String,
    pub(crate) photo: Vec<u8>,
}
