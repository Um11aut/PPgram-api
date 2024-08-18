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

impl ResponseUserDetails {
    pub fn to_json_string(method: &str, response: UserDetails) -> String {
        let details = ResponseUserDetails {
            method: method.into(),
            response: response
        };
        serde_json::to_string(&details).unwrap()
    }
}

pub enum UserIdentifier {
    UserId(i32),
    Username(String)
}

impl<'a> From<&'a str> for UserIdentifier {
    fn from(str: &'a str) -> Self {
        UserIdentifier::Username(str.into())
    }
}

impl From<i32> for UserIdentifier {
    fn from(user_id: i32) -> Self {
        UserIdentifier::UserId(user_id)
    }
}