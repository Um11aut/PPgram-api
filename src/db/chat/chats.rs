use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
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

    async fn create_table(&self) -> Result<(), DatabaseError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS chats (
                id int PRIMARY KEY,
                is_group boolean,
                participants LIST<int>            
            );
        "#;

        self.session.execute(create_table_query).await?;
    
        Ok(())
    }
}

impl ChatsDB {
    pub async fn create_chat(&self, participants: Vec<i32/* user_id */>) -> Result<i32 /* chat_id */, DatabaseError> {
        let chat_id = rand::random::<i32>();
        let insert_query = "INSERT INTO chats (id, is_group, participants) VALUES (?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, participants.len() == 2)?;
        
        let mut list = cassandra_cpp::List::new();
        for participant in participants {
            list.append_int32(participant)?;
        }
        statement.bind_list(2, list)?;

        statement.execute().await.map_err(DatabaseError::from)?;

        Ok(chat_id)
    }

    pub async fn add_participant(&self, chat_id: i32, participant: i32 /* user_id */) -> Result<(), DatabaseError> {
        let current = self.fetch_chat_info(chat_id).await?;

        let is_group = current.participants.len() + 1 > 2;

        let update_query = "UPDATE chats SET participants = participants + ? WHERE id = ?;";
        
        let mut statement = self.session.statement(update_query);
        let mut list = cassandra_cpp::List::new();
        list.append_int32(participant)?;
        statement.bind_list(0, list)?;
        statement.bind_int32(1, chat_id)?;

        statement.execute().await.map_err(DatabaseError::from)?;

        if !is_group {
            let update_is_group_query = "UPDATE chats SET is_group = ? WHERE id = ?;";
            
            let mut statement = self.session.statement(update_is_group_query);
            statement.bind_bool(0, false)?;
            statement.bind_int32(1, chat_id)?;
            
            statement.execute().await.map_err(DatabaseError::from)?;
        }

        Ok(())
    }

    pub async fn fetch_chat_info(&self, chat_id: i32) -> Result<ChatInfo, DatabaseError> {
        let select_query = "SELECT * FROM chats WHERE id = ?";

        let mut statement = self.session.statement(&select_query);
        statement.bind_int32(0, chat_id)?;

        match statement.execute().await {
            Ok(result) => {
                if let Some(row) = result.first_row() {
                    let chat_id: i32 = row.get_by_name("id")?;
                    let is_group: bool = row.get_by_name("is_group")?;

                    let mut iter: SetIterator = row.get_by_name("participants")?;
                    let mut participants: Vec<i32> = vec![];
                    while let Some(participant) = iter.next() {
                        participants.push(participant.get_i32()?);
                    } 

                    return Ok(ChatInfo {
                        chat_id,
                        is_group,
                        participants
                    })
                }
                return Err(DatabaseError::from("Given chat_id not found"))
            },
            Err(err) => return Err(DatabaseError::from(err))
        }
    }

    // Fetches all chats that have a chat with the given user_id
    // pub async fn fetch_chats(&self, user_id: i32) -> Result<Vec<ChatInfo>, DatabaseError> {
    //     let select_query = "SELECT * FROM chats WHERE participants CONTAINS ?";

    //     let mut statement = self.session.statement(select_query);
    //     statement.bind_int32(0, user_id)?;

    //     match statement.execute().await {
    //         Ok(result) => {
    //             let mut users: Vec<ChatInfo> = vec![];

    //             while let Some(chat) = result.iter().next() {
    //                 let mut p: Vec<i32> = vec![];
                    
    //                 let participants: cassandra_cpp::Result<SetIterator> = chat.get_by_name("participants");
    //                 if let Ok(mut participants) = participants {
    //                     while let Some(participant) = participants.next() {
    //                         p.push(participant.get_i32()?);
    //                     }
    //                 }

    //                 let o = ChatInfo {
    //                     id: chat.get_by_name("user_id")?,
    //                     participants: p
    //                 };

    //                 users.push(o)
    //             }

    //             Ok(users)
    //         },
    //         Err(err) => Err(DatabaseError::from(err)),
    //     }
    // }
}