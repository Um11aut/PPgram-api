use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RequestAuthMessage {
    method: String,
    user_id: u64,
    session_id: String
}

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

    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some() && self.user_id.is_some()
    }
}