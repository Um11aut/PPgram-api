use super::message::auth_message::{RequestAuthMessage, RequestLoginMessage, RequestRegisterMessage};

pub struct Session {
    session_id: Option<String>,
    user_id: Option<u64>
}

impl Session {
    pub fn new() -> Session {
        Session {
            session_id: None,
            user_id: None,
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