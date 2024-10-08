
use serde::{Deserialize, Serialize};


use crate::db::{chat::chats::ChatsDB, internal::error::PPError};

use super::user::{User, UserId};

pub type ChatId = i32;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatDetails {
    pub name: String,
    pub chat_id: ChatId,
    pub photo: Option<String>,
    pub username: Option<String>,
}

impl ChatDetails {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn photo(&self) -> Option<&String> {
        self.photo.as_ref()
    }

    pub fn username(&self) -> Option<&String> {
        self.username.as_ref()
    }
}

#[derive(Debug)]
pub struct Chat {
    chat_id: ChatId,
    is_group: bool,
    participants: Vec<User>,
    details: Option<ChatDetails>
}

impl Chat {
    pub async fn new(chats_db: ChatsDB, chat_id: ChatId) -> Result<Self, PPError> {
        let chat = chats_db.fetch_chat(chat_id).await?;

        match chat {
            Some(chat) => Ok(chat),
            None => Err(PPError::from("Failed to find chat with the given chat_id!"))
        }
    }

    pub fn construct(chat_id: i32, is_group: bool, participants: Vec<User>, details: Option<ChatDetails>) -> Self {
        Self {
            chat_id,
            is_group,
            participants,
            details
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

    /// Fetches chat details(`ResponseChatInfo`), which is photo, name of the chat, username, etc.
    /// 
    /// If the chat isn't group(2 people only), it will fetch the info of another user in the chat.
    /// 
    /// If the chat is group, info must be present, it will fetch the chat info.
    pub async fn details(&self, relative_to: &UserId) -> Result<Option<ChatDetails>, PPError> {
        match self.is_group {
            // if not is_group, just take the user info of other participant
            false => {
                if let Some(peer) = self.participants.iter().find(|&participant| participant.user_id() != relative_to.as_i32().unwrap()) {
                    return Ok(Some(ChatDetails{
                        name: peer.name().into(),
                        chat_id: self.chat_id,
                        photo: peer.photo().cloned(),
                        username: Some(peer.username().into())
                    }))
                    } else {
                        return Ok(None)
                    }
            }
            true => {
                Ok(self.details.clone())
            }
        }
    }
}