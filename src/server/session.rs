use std::{error::Error, hash::{Hash, Hasher}, net::SocketAddr};

use log::{debug, error, info};
use serde::Serialize;
use serde_json::Value;

use crate::db::{internal::error::PPError, user::USERS_DB};

use tokio::{net::TcpStream, sync::mpsc};

use super::{connection::{self, Connection, ConnectionType}, message::types::{request::auth::*, user::UserId}};

#[derive(Debug)]
pub struct Session {
    session_id: Option<String>,
    user_id: Option<i32>,
    pub connections: Vec<Connection>
}

impl Session {
    pub fn new(socket: TcpStream) -> Session {
        let main_connection = Connection::new(socket, ConnectionType::MainEvents);
        
        Session {
            session_id: None,
            user_id: None,
            connections: vec![main_connection]
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

    pub fn session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }

    pub async fn connections(&self) -> &Vec<Connection> {
        &self.connections
    }

    pub async fn mpsc_send(&mut self, message: impl Serialize, index: usize) {
        self.connections[index].send(message).await;
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