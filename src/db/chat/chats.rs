use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
use cassandra_cpp::TimestampGen;
use log::debug;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::OnceCell;

use db::internal::error::PPError;

use crate::db;
use crate::db::db::Database;
use crate::db::user::USERS_DB;
use crate::server::message::types::chat::Chat;
use crate::server::message::types::chat::ChatDetails;
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

    async fn create_table(&self) -> Result<(), PPError> {
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
    pub async fn create_chat(&self, participants: Vec<i32/* user_id */>) -> Result<i32 /* chat_id */, PPError> {
        let chat_id = rand::random::<i32>();
        let insert_query = "INSERT INTO chats (id, is_group, participants) VALUES (?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, participants.len() != 2)?;
        
        let mut list = cassandra_cpp::List::new();
        for participant in participants {
            list.append_int32(participant)?;
        }
        statement.bind_list(2, list)?;

        statement.execute().await.map_err(PPError::from)?;

        Ok(chat_id)
    }

    pub async fn add_participant(&self, chat_id: i32, participant: i32 /* user_id */) -> Result<(), PPError> {
        let current = self.fetch_chat_info(chat_id).await?;

        let is_group = current.participants.len() + 1 > 2;

        let update_query = "UPDATE chats SET participants = participants + ? WHERE id = ?;";
        
        let mut statement = self.session.statement(update_query);
        let mut list = cassandra_cpp::List::new();
        list.append_int32(participant)?;
        statement.bind_list(0, list)?;
        statement.bind_int32(1, chat_id)?;

        statement.execute().await.map_err(PPError::from)?;

        if !is_group {
            let update_is_group_query = "UPDATE chats SET is_group = ? WHERE id = ?;";
            
            let mut statement = self.session.statement(update_is_group_query);
            statement.bind_bool(0, false)?;
            statement.bind_int32(1, chat_id)?;
            
            statement.execute().await.map_err(PPError::from)?;
        }

        Ok(())
    }

    pub async fn fetch_chat_info(&self, chat_id: i32) -> Result<Chat, PPError> {
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

                    return Ok(Chat {
                        chat_id,
                        is_group,
                        participants
                    })
                }
                return Err(PPError::from("Given chat_id not found"))
            },
            Err(err) => return Err(PPError::from(err))
        }
    }

    /// Fetches chat details(`ResponseChatInfo`), which is photo, name of the chat, username, etc.
    pub async fn fetch_chat_details(&self, me: i32, chat: &Chat) -> Result<Option<ChatDetails>, PPError> {
        match chat.is_group {
            false => {
                if let Some(&peer_id) = chat.participants.iter().find(|&&participant| participant != me) {
                    let user_info = USERS_DB.get().unwrap().fetch_user(peer_id).await?;

                    if let Some(user_info) = user_info {
                        return Ok(Some(ChatDetails{
                            name: user_info.name,
                            photo: user_info.photo,
                            username: user_info.username
                        }))
                    } else {
                        return Ok(None)
                    }
                }
                return Ok(None)
            }
            true => {
                todo!()
            }
        }
    }
}