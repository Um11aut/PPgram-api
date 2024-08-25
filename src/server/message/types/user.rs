
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{internal::error::PPError, user::USERS_DB};

#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    name: String,
    user_id: i32,
    username: String,
    photo: Option<Vec<u8>>,
}

impl User {
    pub async fn new(id: UserId) -> Result<User, PPError> {
        let user = USERS_DB.get().unwrap().fetch_user(id).await?;
        
        match user {
            Some(user) => Ok(user),
            None => Err(PPError::from("Failed to find chat id!"))
        }
    }

    pub fn construct(name: String, user_id: i32, username: String, photo: Option<Vec<u8>>) -> Self {
        Self {
            name,
            user_id,
            username,
            photo
        }
    }

    pub fn user_id(&self) -> i32 {
        self.user_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn photo(&self) -> &Option<Vec<u8>> {
        &self.photo
    }

    pub fn build_response(&self, method: &str) -> Value {
        json!({
            "ok": true,
            "method": method,
            "data": self
        })
    }
}

pub enum UserId {
    UserId(i32),
    Username(String)
}

impl Clone for UserId {
    fn clone(&self) -> Self {
        match self {
            Self::UserId(user_id) => Self::UserId(user_id.clone()),
            Self::Username(username) => Self::Username(username.clone()),
        }
    }
}

impl<'a> From<&'a str> for UserId {
    fn from(str: &'a str) -> Self {
        UserId::Username(str.into())
    }
}

impl From<i32> for UserId {
    fn from(user_id: i32) -> Self {
        UserId::UserId(user_id)
    }
}

impl UserId {
    pub fn get_i32(&self) -> Option<i32> {
        match *self {
            UserId::UserId(user_id) => Some(user_id),
            UserId::Username(_) => None
        }
    }
}