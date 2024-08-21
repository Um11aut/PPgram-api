use std::{error::Error, hash::{Hash, Hasher}, net::SocketAddr};

use log::{error, info};
use serde::Serialize;
use serde_json::Value;

use crate::db::{internal::error::PPError, user::USERS_DB};

use tokio::sync::mpsc;

use super::message::types::{request::auth::*, user::UserId};

#[derive(Debug)]
pub struct Session {
    session_id: Option<String>,
    user_id: Option<i32>,
    ip_addr: SocketAddr,
    sender: mpsc::Sender<Value>
}

impl Hash for Session {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ip_addr.hash(state);
    }
}

impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        self.ip_addr == other.ip_addr
    }
}

impl Eq for Session {}

impl Session {
    pub fn new(ip_addr: SocketAddr, sender: mpsc::Sender<Value>) -> Session {
        Session {
            session_id: None,
            user_id: None,
            ip_addr,
            sender
        }
    }

    pub async fn auth(&mut self, msg: RequestAuthMessage) -> Result<(), PPError>
    {
        let db = USERS_DB.get().unwrap();
        match db.authenticate(msg.user_id, &msg.session_id).await {
            Ok(_) => {
                self.session_id = Some(msg.session_id);
                self.user_id = Some(msg.user_id);
            }
            Err(err) => {
                return Err(err)
            }
        }

        Ok(())
    }

    pub async fn login(&mut self, msg: RequestLoginMessage) -> Result<(), PPError>
    {
        let db = USERS_DB.get().unwrap();
        match db.login(&msg.username, &msg.password_hash).await {
            Ok((user_id, session_id)) => {
                self.user_id = Some(user_id);
                self.session_id = Some(session_id)
            },
            Err(err) => {
                return Err(err)
            },
        }

        Ok(())
    }

    pub async fn register(&mut self, msg: RequestRegisterMessage) -> Result<(), PPError>
    {
        let db = USERS_DB.get().unwrap();
        match db.register(&msg.name, &msg.username, &msg.password_hash).await {
            Ok((user_id, session_id)) => {
                self.user_id = Some(user_id);
                self.session_id = Some(session_id)
            },
            Err(err) => {
                return Err(err)
            },
        }

        Ok(())
    }

    pub async fn send(&self, message: impl Serialize) {
        self.sender.send(serde_json::to_value(message).unwrap()).await.unwrap();
    } 

    // `(i32, String)` -> user_id, session_id 
    pub fn get_credentials(&self) -> Option<(UserId, String)> {
        if self.is_authenticated() {
            return Some((self.user_id.unwrap().into(), self.session_id.clone().unwrap()))
        }

        None
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some() && self.user_id.is_some()
    }
}