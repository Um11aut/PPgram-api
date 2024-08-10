use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::TimestampGen;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::OnceCell;

use db::internal::error::DatabaseError;

use crate::db;
use crate::db::db::Database;
use crate::server::message::types::chat::ChatInfo;
use crate::server::message::types::user::UserInfo;

pub(crate) static CHATS_DB: OnceCell<ChatsDB> = OnceCell::const_new();

pub(crate) struct ChatsDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for ChatsDB {
    async fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) {
        let create_table_query = r#"
            CREATE TABLE chats (
                chat_id int PRIMARY KEY,
                from_id int,
                peer_id int,
                created_at bigint
            );
        "#;

        match self.session.execute(create_table_query).await {
            Ok(_) => {}
            Err(err) => {
                error!("{}", err);
            }
        }
    }
}

impl ChatsDB {
    pub async fn create_chat(&self, from_id: i32, peer_id: i32) -> Result<i32 /* chat_id */, DatabaseError> {
        let chat_id = rand::random::<i32>();
        let insert_query = "INSERT INTO chats (chat_id, created_at, from_id, peer_id) VALUES (?, ?, ?, ?);";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        
        statement.bind_int64(1, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64)?;

        statement.bind_int32(2, from_id)?;
        statement.bind_int32(3, peer_id)?;

        match statement.execute().await {
            Ok(_) => Ok(chat_id),
            Err(err) => Err(DatabaseError::from(err)),
        }
    }

    // Fetches all chats that have a chat with the given user_id
    pub async fn fetch_chats(&self, user_id: i32) -> Result<Vec<ChatInfo>, DatabaseError> {
        let select_query = "SELECT chat_id, created_at, from_id, peer_id FROM chats WHERE from_id = ? OR peer_id = ? ALLOW FILTERING;";

        let mut statement = self.session.statement(select_query);
        statement.bind_int32(0, user_id)?;
        statement.bind_int32(1, user_id)?;

        match statement.execute().await {
            Ok(result) => {
                let mut users: Vec<ChatInfo> = vec![];

                while let Some(chat) = result.iter().next() {
                    let o = ChatInfo {
                        chat_id: chat.get_by_name("user_id")?,
                        created_at: chat.get_by_name("created_at")?,
                        from_id: chat.get_by_name("from_id")?,
                        peer_id: chat.get_by_name("peer_id")?
                    };

                    users.push(o)
                }

                Ok(users)
            },
            Err(err) => Err(DatabaseError::from(err)),
        }
    }
}