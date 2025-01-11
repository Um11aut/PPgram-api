use serde::{Deserialize, Serialize};

use crate::db::internal::error::PPResult;

use super::user::{User, UserId};

pub type ChatId = i32;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatDetails {
    pub name: String,
    pub chat_id: ChatId,
    pub is_group: bool,
    pub photo: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatDetailsResponse {
    #[serde(flatten)]
    pub details: ChatDetails,
    pub unread_count: u64,
    pub draft: String,
}

impl ChatDetails {
    pub fn construct(
        name: String,
        chat_id: ChatId,
        is_group: bool,
        photo: Option<String>,
        tag: Option<String>,
    ) -> Self {
        Self {
            name,
            chat_id,
            is_group,
            photo,
            tag,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn photo(&self) -> Option<&String> {
        self.photo.as_ref()
    }

    pub fn tag(&self) -> Option<&String> {
        self.tag.as_ref()
    }
}

#[derive(Debug)]
pub struct Chat {
    chat_id: ChatId,
    is_group: bool,
    participants: Vec<User>,
}

impl Chat {
    pub fn construct(chat_id: i32, is_group: bool, participants: Vec<User>) -> Self {
        Self {
            chat_id,
            is_group,
            participants,
        }
    }

    pub fn is_group(&self) -> bool {
        self.is_group
    }

    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    pub fn participants(&self) -> &Vec<User> {
        &self.participants
    }

    /// Group ChatDetails are fetched earlier
    pub async fn get_personal_chat_details(&self, relative_to: &UserId) -> PPResult<ChatDetails> {
        match self.is_group {
            false => {
                if let Some(peer) = self
                    .participants
                    .iter()
                    .find(|&participant| participant.user_id() != relative_to.as_i32_unchecked())
                {
                    Ok(ChatDetails {
                        name: peer.name().into(),
                        chat_id: self.chat_id,
                        is_group: self.is_group,
                        photo: peer.photo().cloned(),
                        tag: Some(peer.username().into()),
                    })
                } else {
                    Err("Provided UserId wasn't found in the chat!".into())
                }
            }
            true => Err("ChatDetails can only be got for a personal chat!".into()),
        }
    }
}
