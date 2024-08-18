use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
use log::error;
use rand::{distributions::Alphanumeric, Rng};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::OnceCell;

use crate::server::message::types::user::UserInfo;
use crate::server::session;

use super::db::Database;
use super::internal::error::PPError;
use super::internal::validate;

pub(crate) static USERS_DB: OnceCell<UsersDB> = OnceCell::const_new();

pub(crate) struct UsersDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for UsersDB {
    async fn new(session: Arc<cassandra_cpp::Session>) -> UsersDB {
        UsersDB { session }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS users (
                id int PRIMARY KEY, 
                name TEXT, 
                username TEXT,
                photo BLOB,
                password_hash TEXT, 
                sessions LIST<TEXT>,
                chats LIST<int>
            )
        "#;

        let create_index_query = r#"
            CREATE INDEX IF NOT EXISTS username_idx ON users (username)
        "#;

        self.session.execute(create_table_query).await?;
        self.session.execute(create_index_query).await?;

        Ok(())
    }
}

impl UsersDB {
    pub async fn username_exists(&self, username: &str) -> Result<bool, PPError> {
        let query = "SELECT id FROM users WHERE username = ?";
        let mut statement = self.session.statement(query);
        statement.bind_string(0, username)?;
        
        let user_exists: bool = match statement.execute().await {
            Ok(result) => result.first_row().is_some(),
            Err(err) => {
                return Err(PPError::from(err));
            }
        };
        
        Ok(user_exists)
    }

    pub async fn user_id_exists(&self, user_id: i32) -> Result<bool, PPError> {
        let query = "SELECT id FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;
        
        let user_exists: bool = match statement.execute().await {
            Ok(result) => result.first_row().is_some(),
            Err(err) => {
                return Err(PPError::from(err));
            }
        };
        
        Ok(user_exists)
    }

    /// Register the user in database. Returns `user_id` and `session_id` if successfull
    pub async fn register(
        &self,
        name: &str,
        username: &str,
        password_hash: &str,
    ) -> std::result::Result<(i32 /* user_id */, String /* session_id */), PPError> {
        validate::validate_name(name)?;
        validate::validate_username(username)?;

        if self.username_exists(username).await? {
            return Err(PPError::from("Username already taken"));
        }

        let user_id = rand::random::<i32>();
        let query = r#"
            INSERT INTO users (id, name, username, password_hash, sessions, photo) VALUES (?, ?, ?, ?, ?, ?)
        "#;
        let mut statement = self.session.statement(query);

        statement.bind_int32(0, user_id)?;
        statement.bind_string(1, name)?;
        statement.bind_string(2, username)?;
        statement.bind_string(3, password_hash)?;
        statement.bind_list(4, cassandra_cpp::List::new())?;
        statement.bind_bytes(5, Vec::new())?;

        statement.execute().await?;

        match self.create_session(user_id).await {
            Ok(session_id) => Ok((user_id, session_id)),
            Err(err) => Err(err),
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password_hash: &str,
    ) -> std::result::Result<(i32 /* user_id */, String /* session_id */), PPError> {
        let query = "SELECT id, password_hash FROM users WHERE username = ?";
        let mut statement = self.session.statement(query);
        statement.bind_string(0, username)?;

        let (user_id, stored_password_hash): (Option<i32>, Option<String>) =
            match statement.execute().await {
                Ok(result) => match result.first_row() {
                    Some(row) => {
                        let user_id: i32 = row.get(0)?;
                        let stored_password_hash: String = row.get(1)?;
                        (Some(user_id), Some(stored_password_hash))
                    }
                    None => (None, None),
                },
                Err(err) => {
                    return Err(PPError::from(err));
                }
            };

        if let (Some(user_id), Some(stored_password_hash)) = (user_id, stored_password_hash) {
            if stored_password_hash != password_hash {
                return Err(PPError::from("Invalid password"));
            }

            match self.create_session(user_id).await {
                Ok(session_id) => Ok((user_id, session_id)),
                Err(err) => Err(err),
            }
        } else {
            Err(PPError::from(
                "User with the given credentials not found!",
            ))
        }
    }

    pub async fn authenticate(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> std::result::Result<(), PPError> {
        let query = "SELECT sessions FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;

        let sessions = match statement.execute().await {
            Ok(result) => {
                if let Some(row) = result.first_row() {
                    let result: cassandra_cpp::Result<cassandra_cpp::SetIterator> = row.get(0);

                    let mut o: Vec<String> = Vec::with_capacity(3);
                    if let Ok(mut sessions) = result {
                        while let Some(session) = sessions.next() {
                            o.push_within_capacity(session.to_string()).unwrap();
                        }
                    }

                    o
                } else {
                    return Err(PPError::from("User not found"))
                }
            }
            Err(err) => {
                error!("{}", err);
                return Err(PPError::from(err));
            }
        };

        if !sessions.is_empty()
        {
            if !sessions.iter().any(|s| s == session_id) {
                return Err(PPError::from("Invalid session"));
            }
        }

        Ok(())
    }

    async fn create_session(&self, user_id: i32) -> Result<String, PPError> {
        let new_session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let query = "SELECT sessions FROM users WHERE id = ?;";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;

        let mut existing_sessions: Vec<String> = match statement.execute().await {
            Ok(result) => {
                let mut o = Vec::with_capacity(3);

                let mut iter = result.iter();
                while let Some(row) = iter.next() {
                    let sessions: cassandra_cpp::Result<SetIterator> = row.get(0);

                    if let Ok(mut sessions) = sessions {
                        while let Some(session) = sessions.next() {
                            o.push_within_capacity(session.to_string()).unwrap();
                        }
                    }
                }

                o
            }
            Err(err) => {
                return Err(PPError::from(err));
            }
        };

        // If sessions array exceeds the maximum size, delete the first one
        if existing_sessions.len() == 3 {
            existing_sessions.drain(..1);
        }

        let query = "UPDATE users SET sessions = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        let mut updated_sessions = cassandra_cpp::List::new();
        for session in existing_sessions {
            updated_sessions.append_string(session.as_str())?;
        }

        updated_sessions.append_string(new_session.as_str())?;

        statement.bind_list(0, updated_sessions)?;
        statement.bind_int32(1, user_id)?;

        match statement.execute().await {
            Ok(_) => Ok(new_session),
            Err(err) => Err(PPError::from(err)),
        }
    }

    pub async fn update_photo<T: Into<Cow<'static, Vec<u8>>>>(
        &self,
        user_id: i32,
        photo: T,
    ) -> std::result::Result<(), PPError> {
        let query = "UPDATE users SET photo = ? WHERE id = ?";
        let mut statement = self.session.statement(query);
        let photo = photo.into().to_vec();

        statement.bind_bytes(0, photo)?;
        statement.bind_int32(1, user_id)?;

        match statement.execute().await {
            Ok(_) => Ok(()),
            Err(err) => Err(PPError::from(err)),
        }
    }

    pub async fn fetch_chats(&self, user_id: i32) -> Result<Vec<i32 /* chat_id */>, PPError> {
        let query = "SELECT chats FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;

        match statement.execute().await {
            Ok(result) => {
                let mut o: Vec<i32> = vec![];

                let mut iter = result.iter();
                while let Some(row) = iter.next() {
                    let chats: cassandra_cpp::Result<SetIterator> = row.get(0);

                    if let Ok(mut chats) = chats {
                        while let Some(chat) = chats.next() {
                            o.push(chat.get_i32()?);
                        }
                    }
                }

                Ok(o)
            }
            Err(err) => Err(PPError::from(err)),
        }
    }

    pub async fn add_chat(&self, user_id: i32, chat_id: i32) -> Result<(), PPError> {
        let query = "SELECT chats FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;
    
        let mut chats: Vec<i32> = match statement.execute().await {
            Ok(result) => {
                let mut o: Vec<i32> = vec![];
                if let Some(row) = result.first_row() {
                    let chat_set: cassandra_cpp::Result<SetIterator> = row.get(0);
                    if let Ok(mut chat_set) = chat_set {
                        while let Some(chat) = chat_set.next() {
                            o.push(chat.get_i32()?);
                        }
                    }
                }
                o
            }
            Err(err) => return Err(PPError::from(err)),
        };
    
        if !chats.contains(&chat_id) {
            chats.push(chat_id);
        }
    
        let query = "UPDATE users SET chats = ? WHERE id = ?";
        let mut statement = self.session.statement(query);
    
        let mut updated_chats = cassandra_cpp::List::new();
        for chat in chats {
            updated_chats.append_int32(chat)?;
        }
    
        statement.bind_list(0, updated_chats)?;
        statement.bind_int32(1, user_id)?;
    
        statement.execute().await?;

        Ok(())
    }
    

    pub async fn fetch_user(&self, user_id: i32) -> Result<Option<UserInfo>, PPError> {
        let query = "SELECT id, name, photo, username FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id)?;

        let result = statement.execute().await?;
        let row = result.first_row();
        if let Some(row) = row {
            let user_id: i32 = row.get(0)?;
            let name: String = row.get(1)?;
            let photo: Vec<u8> = row.get(2)?;
            let username: String = row.get(3)?;

            return Ok(Some(UserInfo{
                name,
                user_id,
                username,
                photo
            }))
        }

        Ok(None)
    }
}
