use std::{error::Error, hash::{Hash, Hasher}, net::SocketAddr};

use super::message::auth_message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage};

#[derive(Debug)]
pub struct Session {
    session_id: Option<String>,
    user_id: Option<String>,
    ip_addr: SocketAddr
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

impl Clone for Session {
    fn clone(&self) -> Self {
        Self { session_id: self.session_id.clone(), user_id: self.user_id.clone(), ip_addr: self.ip_addr.clone() }
    }
}

impl Session {
    pub fn new(ip_addr: SocketAddr) -> Session {
        Session {
            session_id: None,
            user_id: None,
            ip_addr
        }
    }

    pub fn auth(&mut self, msg: RequestAuthMessage) {
        // TODO: implement session checking in DB
        
        self.session_id = Some(msg.session_id);
        self.user_id = Some(msg.user_id);
    }

    pub fn login(&mut self, msg: RequestLoginMessage) {
        // TODO: implement session checking in DB
                
    }

    pub fn register(&mut self, msg: RequestRegisterMessage) {
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some() && self.user_id.is_some()
    }
}