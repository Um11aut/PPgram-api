use std::fmt::{self};

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