use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
use std::sync::Arc;
use tokio::sync::OnceCell;

use db::internal::error::PPError;

use crate::db;
use crate::db::db::Database;
use crate::db::user::USERS_DB;
use crate::server::message::types::chat::Chat;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::User;
use crate::server::message::types::user::UserId;

pub static CHATS_DB: OnceCell<ChatsDB> = OnceCell::const_new();

pub struct ChatsDB {
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
    pub async fn create_chat(&self, participants: Vec<UserId>) -> Result<Chat, PPError> {
        let chat_id = rand::random::<i32>();
        let insert_query = "INSERT INTO chats (id, is_group, participants) VALUES (?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, participants.len() != 2)?;
        
        let mut list = cassandra_cpp::List::new();
        for participant in participants {
            match participant {
                UserId::UserId(user_id) => {
                    list.append_int32(user_id)?;
                }
                UserId::Username(_) => {
                    return Err(PPError::from("Cannot add chat with username!"))
                }
            }
        }
        statement.bind_list(2, list)?;

        statement.execute().await.map_err(PPError::from)?;

        Ok(self.fetch_chat(chat_id).await.unwrap().unwrap())
    }

    pub async fn add_participant(&self, chat_id: ChatId, participant: UserId) -> Result<(), PPError> {
        let current = match self.fetch_chat(chat_id).await? {
            Some(current) => current,
            None => return Err(PPError::from("Given Chat Id wasn't found!"))
        };

        let is_group = current.participants().len() + 1 > 2;

        let update_query = "UPDATE chats SET participants = participants + ? WHERE id = ?;";
        
        let mut statement = self.session.statement(update_query);
        let mut list = cassandra_cpp::List::new();
        match participant {
            UserId::UserId(user_id) => {
                list.append_int32(user_id)?;
            }
            UserId::Username(_) => {
                return Err(PPError::from("UserId must be integer!"))
            }
        }
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

    pub async fn fetch_chat(&self, chat_id: ChatId) -> Result<Option<Chat>, PPError> {
        let select_query = "SELECT * FROM chats WHERE id = ?";

        let mut statement = self.session.statement(&select_query);
        statement.bind_int32(0, chat_id)?;

        match statement.execute().await {
            Ok(result) => {
                if let Some(row) = result.first_row() {
                    let chat_id: i32 = row.get_by_name("id")?;
                    let is_group: bool = row.get_by_name("is_group")?;

                    let mut iter: SetIterator = row.get_by_name("participants")?;
                    let users_db = USERS_DB.get().unwrap();
                    
                    let mut participants: Vec<User> = vec![];
                    while let Some(participant) = iter.next() {
                        let user = users_db.fetch_user(participant.get_i32()?.into()).await?;
                        if let Some(user) = user {
                            participants.push(user)
                        }
                    }

                    return Ok(Some(Chat::construct(
                        chat_id,
                        is_group,
                        participants
                    )))
                }
                return Ok(None)
            },
            Err(err) => return Err(PPError::from(err))
        }
    }
}