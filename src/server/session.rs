use std::net::SocketAddr;

pub struct Session {
    uuid: u128,
    ip: SocketAddr
}

impl Session {
    pub fn new(ip: SocketAddr) -> Session {
        // TODO: implement uuid checking in DB
        Session {
            uuid: 0,
            ip: ip
        }
    }

    pub fn is_authenticated(&self) -> bool {
        true
    }
}