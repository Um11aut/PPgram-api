use std::{fmt::{self}, sync::Arc};

use log::error;
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::server::message::types::error::error::PPgramError;

#[derive(Debug)]
pub enum DatabaseError {
    Cassandra(cassandra_cpp::Error),
    Client(String)
}

unsafe impl Send for DatabaseError {}
unsafe impl Sync for DatabaseError {}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DatabaseError::Cassandra(ref err) => write!(f, "{}", err),
            DatabaseError::Client(ref msg) => write!(f, "{}", msg)
        }
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            DatabaseError::Cassandra(ref err) => Some(err),
            DatabaseError::Client(_) => None,
        }
    }
}

impl From<cassandra_cpp::Error> for DatabaseError {
    fn from(err: cassandra_cpp::Error) -> Self {
        DatabaseError::Cassandra(err)
    }
}

impl From<String> for DatabaseError {
    fn from(err: String) -> Self {
        DatabaseError::Client(err)
    }
}

impl From<&str> for DatabaseError {
    fn from(value: &str) -> Self {
        DatabaseError::from(String::from(value))
    }
}

impl DatabaseError {
    /// if Cassandra error, writes error to console and sends 'Internal error.' to user.
    /// 
    /// if Client error, sends error to the client
    pub async fn safe_send(&self, method: &str, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let err: String = match self {
            DatabaseError::Cassandra(internal) => {
                error!("{}", internal);
                "Internal error.".into()
            }
            DatabaseError::Client(_) => {
                self.to_string()
            }
        };
        PPgramError::send(method, err, Arc::clone(&writer)).await;
    }
}