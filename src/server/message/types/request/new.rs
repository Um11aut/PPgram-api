use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NewGroupRequest {
    pub method: String, // new
    pub what: String, // group
    pub name: String, // SomeName123
    pub avatar_hash: Option<String>,
    pub username: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NewInvitationLinkRequest {
    pub method: String, // new
    pub what: String, // invitation_link
    pub chat_id: i32
}