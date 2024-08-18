use std::{fmt::{self}, sync::Arc};

use log::error;
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::server::message::types::error::error::PPErrorSender;

#[derive(Debug)]
pub enum PPError {
    Cassandra(cassandra_cpp::Error),
    Client(String)
}

unsafe impl Send for PPError {}
unsafe impl Sync for PPError {}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PPError::Cassandra(ref err) => write!(f, "{}", err),
            PPError::Client(ref msg) => write!(f, "{}", msg)
        }
    }
}

impl std::error::Error for PPError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            PPError::Cassandra(ref err) => Some(err),
            PPError::Client(_) => None,
        }
    }
}

impl From<cassandra_cpp::Error> for PPError {
    fn from(err: cassandra_cpp::Error) -> Self {
        PPError::Cassandra(err)
    }
}

impl From<String> for PPError {
    fn from(err: String) -> Self {
        PPError::Client(err)
    }
}

impl From<&str> for PPError {
    fn from(value: &str) -> Self {
        PPError::from(String::from(value))
    }
}

impl PPError {
    /// if Cassandra error, writes error to console and sends 'Internal error.' to user.
    /// 
    /// if Client error, sends error to the client
    pub async fn safe_send(&self, method: &str, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let err: String = match self {
            PPError::Cassandra(internal) => {
                error!("{}", internal);
                "Internal error.".into()
            }
            PPError::Client(_) => {
                self.to_string()
            }
        };
        PPErrorSender::send(method, err, Arc::clone(&writer)).await;
    }
}