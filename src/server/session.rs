use std::sync::Arc;

use serde::Serialize;

use crate::db::{bucket::{DatabaseBucket, DatabaseBuilder}, internal::error::{PPError, PPResult}, user::UsersDB};

use tokio::net::TcpStream;

use super::{connection::TCPConnection, message::types::{request::auth::*, user::UserId}};

/// component for authenticated the `Session`
pub struct AuthComponent {
    session_id: String,
    user_id: i32
}

impl AuthComponent {
    pub async fn from_auth(db: UsersDB, req: AuthRequest) -> PPResult<Self> {
        match db.authenticate(req.user_id, &req.session_id).await {
            Ok(_) => Ok(Self {
                session_id: req.session_id,
                user_id: req.user_id
            }),
            Err(err) => Err(err)
        }
    }
    
    pub async fn from_login(db: UsersDB, req: LoginRequest) -> PPResult<Self> {
        match db.login(&req.username, &req.password).await {
            Ok((user_id, session_id)) => Ok(Self{
                session_id,
                user_id
            }),
            Err(err) => Err(err)
        }
    }

    pub async fn from_register(db: UsersDB, req: RegisterRequest) -> PPResult<Self> {
        match db.register(&req.name, &req.username, &req.password).await {
            Ok((user_id, session_id)) => Ok(Self{
                session_id,
                user_id
            }),
            Err(err) => Err(err)
        }
    }

    pub fn get_credentials(self) -> (String, i32) {
        (self.session_id, self.user_id)
    }
}

#[derive(Debug)]
pub struct Session {
    session_id: Option<String>,
    user_id: Option<i32>,
    connections: Vec<Arc<TCPConnection>>
}

impl Session {
    pub fn new(socket: TcpStream) -> Session {
        let main_connection = TCPConnection::new(socket);
        
        Session {
            session_id: None,
            user_id: None,
            connections: vec![Arc::new(main_connection)]
        }
    }

    /// Session can be authenticated for basic functionality
    /// 
    /// e.g. sending messages, fetching
    pub fn authenticate(&mut self, auth_component: AuthComponent)
    {
        let (s_id, u_id) = auth_component.get_credentials();
        self.session_id = Some(s_id);
        self.user_id = Some(u_id);
    }

    pub fn session_id(&self) -> Option<&String> {
        self.session_id.as_ref()
    }

    pub fn connections(&self) -> &Vec<Arc<TCPConnection>> {
        &self.connections
    }

    /// In case of binding to existing session(see methods)
    pub fn add_connection(&mut self, connection: Arc<TCPConnection>) {
        self.connections.push(connection);
    }

    /// In case of disconnecting
    pub fn remove_connection(&mut self, connection: Arc<TCPConnection>) {
        self.connections.retain(|x| Arc::ptr_eq(x, &connection));
    }

    /// Sends json message to some existing connection (with connection index as server can handle multiple connections per session)
    /// 
    /// Needed for live events, sending messages etc.
    pub async fn mpsc_send(&mut self, message: impl Serialize, con_idx: usize) {
        self.connections[con_idx].send(message).await;
    }

    /// `(UserId, String)` -> user_id, session_id 
    pub fn get_credentials(&self) -> Option<(UserId, String)> {
        if self.is_authenticated() {
            return Some((self.user_id.unwrap().into(), self.session_id.clone().unwrap()))
        }

        None
    }

    /// Should be used if authentification was required earlier in the code 
    /// 
    /// Because we know that user is already authenticated
    pub fn get_credentials_unchecked(&self) -> (UserId, String) {
        return (self.user_id.unwrap().into(), self.session_id.clone().unwrap())
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some() && self.user_id.is_some()
    }
}