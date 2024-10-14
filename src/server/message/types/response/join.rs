use serde::{Deserialize, Serialize};

use crate::server::message::types::chat::ChatDetails;

#[derive(Serialize, Deserialize)]
pub struct JoinGroupResponse {
    pub ok: bool, // true
    pub method: String, // join
    pub chat: ChatDetails
}

/// Workaround
/// 
/// Not to send error, but rather to show that invitation link isn't valid
#[derive(Serialize, Deserialize)]
pub struct JoinLinkNotFoundResponse {
    pub ok: bool, // true
    pub method: String, // join
    pub code: u64 // 404
}