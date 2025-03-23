use std::{
    borrow::Cow,
    fmt::{self},
};

use log::error;
use scylla::{deserialize::TypeCheckError, transport::errors::QueryError};
use serde_json::json;

use crate::server::{connection::TCPConnection, message::builder::MessageBuilder};

async fn send_str_as_err<T: Into<Cow<'static, str>>>(
    method: &str,
    what: T,
    req_id: Option<i64>,
    connection: &TCPConnection,
) {
    let what: String = what.into().to_string();

    let mut error = json!({
        "ok": false,
        "method": method,
        "error": what,
    });
    let obj = error.as_object_mut().unwrap();
    if let Some(req_id) = req_id {
        obj.insert("req_id".into(), req_id.into());
    }

    let builder = MessageBuilder::build_from_str(serde_json::to_string(&error).unwrap());

    connection.write(&builder.packed()).await;
}

/// The error struct that represents all possible
/// errors that may occur in the code
#[derive(Debug)]
pub enum PPError {
    Server(Box<dyn std::error::Error>),
    Client(String),
}

unsafe impl Send for PPError {}
unsafe impl Sync for PPError {}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PPError::Server(ref err) => write!(f, "INTERNAL ERROR: {}", err),
            PPError::Client(ref msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PPError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            PPError::Server(ref err) => Some(err.as_ref()),
            PPError::Client(_) => None,
        }
    }
}

impl From<tokio::io::Error> for PPError {
    fn from(value: tokio::io::Error) -> Self {
        PPError::Server(Box::new(value))
    }
}

impl From<TypeCheckError> for PPError {
    fn from(err: TypeCheckError) -> Self {
        PPError::Server(Box::new(err))
    }
}

impl From<QueryError> for PPError {
    fn from(err: QueryError) -> Self {
        PPError::Server(Box::new(err))
    }
}

impl From<serde_json::Error> for PPError {
    fn from(err: serde_json::Error) -> Self {
        PPError::Client(format! {"error while parsing json: {}", err})
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
    /// if Server error, writes error to console and sends 'Internal error.' to user.
    ///
    /// if Client error, sends error to the client
    pub async fn safe_send(
        &self,
        method: &str,
        req_id: Option<i64>,
        output_connection: &TCPConnection,
    ) {
        let err: String = match self {
            PPError::Server(internal) => {
                error!("{}", internal);
                "Internal error.".into()
            }
            PPError::Client(_) => self.to_string(),
        };
        send_str_as_err(method, err, req_id, output_connection).await;
    }
}
pub type PPResult<T> = Result<T, PPError>;
