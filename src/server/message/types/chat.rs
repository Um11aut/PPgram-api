use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::db::{chat::chats::CHATS_DB, internal::error::PPError, user::USERS_DB};

use super::user::{User, UserId};

pub type ChatId = i32;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatDetails {
    name: String,
    photo: Option<Vec<u8>>,
    username: String,
}

impl ChatDetails {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn photo(&self) -> &Option<Vec<u8>> {
        &self.photo
    }

    pub fn username(&self) -> &str {
        &self.username
    }
}

pub struct Chat {
    chat_id: ChatId,
    is_group: bool,
    participants: Vec<User>,
    details: Option<ChatDetails>
}

impl Chat {
    pub async fn new(chat_id: ChatId) -> Result<Self, PPError> {
        let chat = CHATS_DB.get().unwrap().fetch_chat(chat_id).await?;

        match chat {
            Some(chat) => Ok(chat),
            None => Err(PPError::from("Failed to find chat with the given chat_id!"))
        }
    }

    pub fn construct(chat_id: i32, is_group: bool, participants: Vec<User>) -> Self {
        Self {
            chat_id,
            is_group,
            participants,
            details: None
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
    pub async fn details(&self, me: &UserId) -> Result<Option<ChatDetails>, PPError> {
        match self.is_group {
            // if not is_group, just take the account of other participant
            false => {
                if let Some(peer) = self.participants.iter().find(|&participant| participant.user_id() != me.get_i32().unwrap()) {
                    return Ok(Some(ChatDetails{
                        name: peer.name().into(),
                        photo: peer.photo().clone(),
                        username: peer.username().to_owned()
                    }))
                    } else {
                        return Ok(None)
                    }
            }
            true => {
                Ok(None)
            }
        }
    }
}