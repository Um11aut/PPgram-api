use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::db::db::Database;

pub(crate) static MESSAGES_DB: OnceCell<MessagesDB> = OnceCell::const_new();

pub(crate) struct MessagesDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for MessagesDB {
    async fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS messages (
                id int PRIMARY KEY, 
                is_unread boolean,
                from_id int,
                chat_id int,
                date timestamp,
                reply_to int,
            )
        "#;

        match self.session.execute(create_table_query).await {
            Ok(_) => {}
            Err(err) => {
                error!("{}", err);
            }
        }
    }
}

impl MessagesDB {
    
}