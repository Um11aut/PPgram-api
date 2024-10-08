use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::OnceCell;

use db::internal::error::PPError;

use crate::db;
use crate::db::connection::DatabaseBucket;
use crate::db::connection::DatabaseBuilder;
use crate::db::db::Database;
use crate::db::internal::error::PPResult;
use crate::db::user::UsersDB;
use crate::server::message::types::chat::Chat;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::User;
use crate::server::message::types::user::UserId;

pub struct ChatsDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for ChatsDB {
    fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS chats (
                id int PRIMARY KEY,
                is_group boolean,
                participants LIST<int>,
                name TEXT,
                avatar_hash TEXT,
                username TEXT
            );
        "#;

        self.session.execute(create_table_query).await?;
    
        Ok(())
    }
}

impl From<DatabaseBucket> for ChatsDB {
    fn from(value: DatabaseBucket) -> Self {
        value.into()
    }
}

impl ChatsDB {
    pub async fn create_private(&self, participants: Vec<UserId>) -> Result<Chat, PPError> {
        let chat_id = rand::thread_rng().gen_range(i32::MIN..-1);
        let insert_query = "INSERT INTO chats (id, is_group, participants) VALUES (?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, false)?;
        
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

    pub async fn create_group(&self, participants: Vec<UserId>, details: ChatDetails) -> Result<Chat, PPError> {
        let chat_id = rand::thread_rng().gen_range(i32::MIN..-1);
        let insert_query = "INSERT INTO chats (id, is_group, participants, name, avatar_hash, username) VALUES (?, ?, ?, ?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, true)?;
        
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
        statement.bind_string(3, details.name())?;
        statement.bind_string(4, details.photo().map_or("", |v| v))?;
        statement.bind_string(5, details.username().map_or("", |v| v))?;

        statement.execute().await?;

        Ok(self.fetch_chat(chat_id).await.unwrap().unwrap())
    }

    pub async fn add_participant(&self, chat_id: ChatId, participant: UserId) -> Result<(), PPError> {
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

        statement.execute().await?;

        Ok(())
    }

    pub async fn chat_exists(&self, chat_id: ChatId) -> PPResult<bool> {
        let query = "SELECT * FROM chats WHERE id = ?";

        let mut statement = self.session.statement(&query);
        statement.bind_int32(0, chat_id)?;
        let res = statement.execute().await?;

        Ok(res.first_row().is_some())
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
                    let users_db: UsersDB = DatabaseBuilder::from_raw(self.session.clone()).into();
                    
                    let mut participants: Vec<User> = vec![];
                    while let Some(participant) = iter.next() {
                        let user = users_db.fetch_user(&participant.get_i32()?.into()).await?;
                        if let Some(user) = user {
                            participants.push(user)
                        }
                    }

                    if is_group {
                        let name: String = row.get_by_name("name")?;
                        let avatar_hash: String = row.get_by_name("avatar_hash")?;
                        let username: String = row.get_by_name("username")?;

                        let details: ChatDetails = ChatDetails {
                            name,
                            chat_id,
                            photo: if !avatar_hash.is_empty(){Some(avatar_hash)}else{None},
                            username: if !username.is_empty(){Some(username)}else{None}
                        };
                        return Ok(Some(Chat::construct(
                            chat_id,
                            is_group,
                            participants,
                            Some(details)
                        )))
                    }

                    return Ok(Some(Chat::construct(
                        chat_id,
                        is_group,
                        participants,
                        None
                    )))
                }
                return Ok(None)
            },
            Err(err) => return Err(err.into())
        }
    }
}

impl From<DatabaseBuilder> for ChatsDB {
    fn from(value: DatabaseBuilder) -> Self {
        ChatsDB {
            session: value.bucket.get_connection()
        }
    }
}