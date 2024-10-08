use std::sync::Arc;

use serde::Serialize;

use crate::db::{connection::{DatabaseBucket, DatabaseBuilder}, internal::error::PPError, user::UsersDB};

use tokio::net::TcpStream;

use super::{connection::Connection, message::types::{request::auth::*, user::UserId}};

// TODO: create authcomponent instead of using all 3 methods directly on session
// pub struct AuthComponent {
    // session_id: String,
    // user_id: i32
// }

// impl From<AuthRequest> for AuthComponent {

// }

#[derive(Debug)]
pub struct Session {
    session_id: Option<String>,
    user_id: Option<i32>,
    connections: Vec<Arc<Connection>>
}

impl Session {
    pub fn new(socket: TcpStream) -> Session {
        let main_connection = Connection::new(socket);
        
        Session {
            session_id: None,
            user_id: None,
            connections: vec![Arc::new(main_connection)]
        }
    }

    pub async fn auth(&mut self, db: UsersDB, msg: AuthRequest) -> Result<(), PPError>
    {
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

    pub async fn login(&mut self, db: UsersDB, msg: LoginRequest) -> Result<(), PPError>
    {
        match db.login(&msg.username, &msg.password).await {
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

    pub async fn register(&mut self, db: UsersDB, msg: RegisterRequest) -> Result<(), PPError>
    {
        match db.register(&msg.name, &msg.username, &msg.password).await {
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

    pub fn connections(&self) -> &Vec<Arc<Connection>> {
        &self.connections
    }

    pub fn get_connection_idx(&self, connection: Arc<Connection>) -> Option<usize> {
        for idx in 0..self.connections.len() {
            if Arc::ptr_eq(&self.connections[idx], &connection) {
                return Some(idx)
            }
        }

        None
    }

    pub fn add_connection(&mut self, connection: Arc<Connection>) {
        self.connections.push(connection);
    }

    pub fn remove_connection(&mut self, connection: Arc<Connection>) {
        self.connections.retain(|x| Arc::ptr_eq(x, &connection));
    }

    /// Sends json message to some connection index()
    pub async fn mpsc_send(&mut self, message: impl Serialize, con_idx: usize) {
        self.connections[con_idx].send(message).await;
    }

    /// `(i32, String)` -> user_id, session_id 
    pub fn get_credentials(&self) -> Option<(UserId, String)> {
        if self.is_authenticated() {
            return Some((self.user_id.unwrap().into(), self.session_id.clone().unwrap()))
        }

        None
    }

    /// Should be used if authentification was required earlier in the code 
    pub fn get_credentials_unchecked(&self) -> (UserId, String) {
        return (self.user_id.unwrap().into(), self.session_id.clone().unwrap())
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some() && self.user_id.is_some()
    }
}