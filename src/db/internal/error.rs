use std::{borrow::Cow, fmt::{self}};

use log::error;
use serde_json::json;

use crate::server::{connection::Connection, message::builder::MessageBuilder};

async fn send_str_as_err<T: Into<Cow<'static, str>>>(method: &str, what: T, connection: &Connection) {
    let what: String = what.into().to_string();

    let error = json!({
        "ok": false,
        "method": method,
        "error": what
    });

    let builder = MessageBuilder::build_from_str(serde_json::to_string(&error).unwrap());

    connection.write(&builder.packed()).await;
}

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

impl From<serde_json::Error> for PPError {
    fn from(err: serde_json::Error) -> Self {
        PPError::Client(format!("error while parsing json: {}", err.to_string()))
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
    pub async fn safe_send(&self, method: &str, connection: &Connection) {
        let err: String = match self {
            PPError::Cassandra(internal) => {
                error!("{}", internal);
                "Internal error.".into()
            }
            PPError::Client(_) => {
                self.to_string()
            }
        };
        send_str_as_err(method, err, connection).await;
    }
}

pub type PPResult<T> = Result<T, PPError>;