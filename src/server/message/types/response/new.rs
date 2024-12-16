use serde::{Deserialize, Serialize};

use crate::server::message::types::chat::{ChatDetails, ChatDetailsResponse};

#[derive(Serialize, Deserialize)]
pub struct NewGroupResponse {
    pub ok: bool, // true
    pub method: String, // new_group
    pub chat: ChatDetailsResponse
}

#[derive(Serialize, Deserialize)]
pub struct NewInvitationLinkResponse {
    pub ok: bool, // true
    pub method: String, // new_invitation_link
    pub link: String // +SDJvnd
}
