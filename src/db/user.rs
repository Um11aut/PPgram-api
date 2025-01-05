use argon2::{
    password_hash::{
        self, rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::MapIterator;
use cassandra_cpp::SetIterator;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::User;
use crate::server::message::types::user::UserId;

use super::{bucket::DatabaseBuilder, chat::hashes::HashesDB};
use super::db::Database;
use super::internal::error::PPError;
use super::internal::error::PPResult;
use super::internal::validate;

pub struct UsersDB {
    session: Arc<cassandra_cpp::Session>,
}

impl From<DatabaseBuilder> for UsersDB {
    fn from(value: DatabaseBuilder) -> Self {
        UsersDB {
            session: value.bucket.get_connection(),
        }
    }
}

impl Database for UsersDB {
    fn new(session: Arc<cassandra_cpp::Session>) -> UsersDB {
        UsersDB { session }
    }

    async fn create_table(&self) -> PPResult<()> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS ksp.users (
                id int PRIMARY KEY,
                name TEXT,
                username TEXT,
                photo TEXT,
                password_hash TEXT,
                password_salt TEXT,
                sessions LIST<TEXT>,
                chats MAP<int, int>
            )
        "#;

        let create_index_query = r#"
            CREATE INDEX IF NOT EXISTS username_idx ON ksp.users (username)
        "#;

        let username_custom_index_query = r#"
            CREATE CUSTOM INDEX IF NOT EXISTS username_sasi_idx ON ksp.users (username)
            USING 'org.apache.cassandra.index.sasi.SASIIndex'
            WITH OPTIONS = {
                'mode': 'CONTAINS',
                'analyzer_class': 'org.apache.cassandra.index.sasi.analyzer.NonTokenizingAnalyzer',
                'case_sensitive': 'false'
            };
        "#;

        let name_custom_index_query = r#"
            CREATE CUSTOM INDEX IF NOT EXISTS name_sasi_idx ON ksp.users (name)
            USING 'org.apache.cassandra.index.sasi.SASIIndex'
            WITH OPTIONS = {
                'mode': 'CONTAINS',
                'analyzer_class': 'org.apache.cassandra.index.sasi.analyzer.NonTokenizingAnalyzer',
                'case_sensitive': 'false'
            };
        "#;

        self.session.execute(create_table_query).await?;
        self.session.execute(create_index_query).await?;
        self.session.execute(username_custom_index_query).await?;
        self.session.execute(name_custom_index_query).await?;

        Ok(())
    }
}

impl UsersDB {
    pub async fn exists(&self, identifier: &UserId) -> PPResult<bool> {
        let result = match identifier {
            UserId::UserId(user_id) => {
                let query = "SELECT id FROM ksp.users WHERE id = ?";
                let mut statement = self.session.statement(query);
                statement.bind_int32(0, *user_id)?;
                statement.execute().await?
            }
            UserId::Username(username) => {
                let query = "SELECT id FROM ksp.users WHERE username = ?";
                let mut statement = self.session.statement(query);
                statement.bind_string(0, username.as_str())?;
                statement.execute().await?
            }
        };

        Ok(result.first_row().is_some())
    }

    /// Register the user in database. Returns `user_id` and `session_id` if successfull
    pub async fn register(
        &self,
        name: &str,
        username: &str,
        password: &str,
    ) -> PPResult<(i32 /* user_id */, String /* session_id */)> {
        validate::validate_name(name)?;
        validate::validate_username(username)?;

        if self.exists(&username.into()).await? {
            return Err(PPError::from("Username already taken"));
        }

        let user_id: i32 = rand::thread_rng().gen_range(1..i32::MAX);
        let query = r#"
            INSERT INTO ksp.users (id, name, username, password_hash, password_salt, sessions, photo, chats) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        let mut statement = self.session.statement(query);

        statement.bind_int32(0, user_id)?;
        statement.bind_string(1, name)?;
        statement.bind_string(2, username)?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| PPError::from(format!("Failed to hash password: {}", err)))?
            .to_string();
        info!("Generated new password hash: {}", password_hash);

        statement.bind_string(3, &password_hash)?;
        statement.bind_string(4, salt.as_str())?;

        statement.bind_list(5, cassandra_cpp::List::new())?;
        statement.bind_string(6, "")?;
        statement.bind_map(7, cassandra_cpp::Map::new())?;

        statement.execute().await?;

        match self.create_session(user_id).await {
            Ok(session_id) => Ok((user_id, session_id)),
            Err(err) => Err(err),
        }
    }

    /// Fetches all users by the given search query
    ///
    /// Restricts the search by max. of 50 users.
    ///
    /// When given str starts with '@', will search by username,
    /// or else by name
    pub async fn fetch_users_by_search_query(
        &self,
        query: impl Into<Cow<'static, str>>,
    ) -> PPResult<Vec<User>> {
        let search_query: String = query.into().to_string();
        if search_query.is_empty() {
            return Ok(vec![]);
        }
        let mut cassandra_search_query: String = format!("%{}%", search_query);

        let cassandra_query = match cassandra_search_query.contains("@") {
            true => {
                if search_query.len() < 2 {
                    return Ok(vec![]);
                }
                // Drain '@' symbol
                cassandra_search_query.remove(1);
                "SELECT * FROM ksp.users WHERE username LIKE ? LIMIT 50;"
            }
            false => "SELECT * FROM ksp.users WHERE name LIKE ? LIMIT 50;",
        };
        let mut statement = self.session.statement(cassandra_query);
        statement.bind_string(0, &cassandra_search_query)?;

        let result = statement.execute().await?;

        let mut o = vec![];
        let mut iter = result.iter();
        while let Some(row) = iter.next() {
            let username: String = row.get_by_name("username")?;
            let user_id: i32 = row.get_by_name("id")?;
            let photo: String = row.get_by_name("photo")?;
            let name: String = row.get_by_name("name")?;

            o.push(User::construct(
                name,
                user_id,
                username,
                if photo.is_empty() { None } else { Some(photo) },
            ));
        }

        Ok(o)
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> PPResult<(i32 /* user_id */, String /* session_id */)> {
        let query = "SELECT id, password_hash FROM ksp.users WHERE username = ?";
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
            let password_matches = Argon2::default()
                .verify_password(
                    password.as_bytes(),
                    &PasswordHash::new(&stored_password_hash)
                        .expect("Failed to parse password hash!"),
                )
                .is_ok();
            if !password_matches {
                return Err(PPError::from("Invalid password!"));
            }

            match self.create_session(user_id).await {
                Ok(session_id) => Ok((user_id, session_id)),
                Err(err) => Err(err),
            }
        } else {
            Err(PPError::from("User with the given credentials not found!"))
        }
    }

    pub async fn authenticate(&self, user_id: i32, session_id: &str) -> PPResult<()> {
        let query = "SELECT sessions FROM ksp.users WHERE id = ?";
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
                    return Err(PPError::from("User not found"));
                }
            }
            Err(err) => {
                error!("{}", err);
                return Err(PPError::from(err));
            }
        };

        if !sessions.is_empty() && !sessions.iter().any(|s| s == session_id) {
            return Err(PPError::from("Invalid session"));
        }

        Ok(())
    }

    async fn create_session(&self, user_id: i32) -> PPResult<String> {
        let new_session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let query = "SELECT sessions FROM ksp.users WHERE id = ?;";
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

        let query = "UPDATE ksp.users SET sessions = ? WHERE id = ?";
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

    pub async fn update_photo(&self, self_user_id: &UserId, media_hash: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET photo = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        statement.bind_string(0, media_hash)?;
        statement.bind_int32(1, self_user_id.as_i32_unchecked())?;

        statement.execute().await?;

        Ok(())
    }

    pub async fn update_name(&self, self_user_id: &UserId, name: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET name = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        statement.bind_string(0, name)?;
        statement.bind_int32(1, self_user_id.as_i32_unchecked())?;

        statement.execute().await?;

        Ok(())
    }

    pub async fn update_username(&self, self_user_id: &UserId, username: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET username = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        statement.bind_string(0, username)?;
        statement.bind_int32(1, self_user_id.as_i32_unchecked())?;

        statement.execute().await?;

        Ok(())
    }

    pub async fn update_password(&self, self_user_id: &UserId, password: &str) -> PPResult<()> {
        // Generate a new salt
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        // Hash the new password with the generated salt
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| PPError::from(format!("Failed to hash password: {}", err)))?
            .to_string();
        info!("Generated new password hash: {}", password_hash);

        let query = "UPDATE ksp.users SET password_hash = ?, password_salt = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        // Bind the new password hash and salt
        statement.bind_string(0, &password_hash)?;
        statement.bind_string(1, salt.as_str())?;
        statement.bind_int32(2, self_user_id.as_i32_unchecked())?;

        statement.execute().await.map_err(|err| {
            error!("Failed to update password: {}", err);
            PPError::from(err)
        })?;

        Ok(())
    }

    /// fetches every chat by given user_id
    ///
    /// where i32 is the UserId
    pub async fn fetch_chats(&self, user_id: &UserId) -> PPResult<HashMap<i32, ChatId>> {
        let statement = match user_id {
            UserId::UserId(user_id) => {
                let query = "SELECT chats FROM ksp.users WHERE id = ?";
                let mut statement = self.session.statement(query);
                statement.bind_int32(0, *user_id)?;
                statement
            }
            UserId::Username(username) => {
                let query = "SELECT chats FROM ksp.users WHERE username = ?";
                let mut statement = self.session.statement(query);
                statement.bind_string(0, username)?;
                statement
            }
        };
        let result = statement.execute().await?;

        let mut output: HashMap<i32, ChatId> = HashMap::new();

        let mut iter = result.iter();
        while let Some(row) = iter.next() {
            let maybe_iter: cassandra_cpp::Result<MapIterator> = row.get(0);

            if let Ok(mut chats) = maybe_iter {
                while let Some((key, val)) = chats.next() {
                    output.insert(key.get_i32()?, val.get_i32()?);
                }
            }
        }

        Ok(output)
    }

    /// In Users Database, the chat id is stored in a map of public chat_ids(for each user it's relative), and private(server) chat_ids
    ///
    /// This is made for simplicity of API usage, meaning that if e.g. user wants to write a message to some `user_id`,
    /// user shouldn't bother and fetch some other `chat_id` for his intentions, he can just send an intended message to
    /// the exact `chat_id`(`user_id`), that was fetched by him before
    ///
    ///
    /// This function gets associated private `chat_id` from by the public `user_id`(`chat_id`) key
    pub async fn get_associated_chat_id(
        &self,
        self_user_id: &UserId,
        key_chat_id: ChatId,
    ) -> PPResult<Option<ChatId>> {
        let mut statement = match self_user_id {
            UserId::UserId(user_id) => {
                let query = "SELECT chats[?] FROM ksp.users WHERE id = ?";
                let mut statement = self.session.statement(query);
                statement.bind_int32(1, *user_id)?;
                statement
            }
            UserId::Username(username) => {
                let query = "SELECT chats[?] FROM ksp.users WHERE username = ?";
                let mut statement = self.session.statement(query);
                statement.bind_string(1, username)?;
                statement
            }
        };
        statement.bind_int32(0, key_chat_id)?;

        let result = statement.execute().await?;

        if let Some(row) = result.first_row() {
            let maybe_chat_id: cassandra_cpp::Result<i32> = row.get(0);

            if let Ok(associated_chat_id) = maybe_chat_id {
                return Ok(Some(associated_chat_id));
            }
        }

        Ok(None)
    }

    /// Adds to a `chats` map new `Key, Value`
    ///
    /// Key is public chat id that is relative and visible to the self user
    ///
    /// `private_chat_id` is the real chat id.
    pub async fn add_chat(
        &self,
        self_user_id: &UserId,
        public_chat_id: ChatId,
        private_chat_id: ChatId,
    ) -> PPResult<()> {
        let query = match self_user_id {
            UserId::UserId(_) => "UPDATE ksp.users SET chats = chats + ? WHERE id = ?",
            UserId::Username(_) => "UPDATE ksp.users SET chats = chats + ? WHERE username = ?",
        };

        let mut statement = self.session.statement(query);

        // Create a list with a single chat_id to append to the chats list
        let mut chat_list = cassandra_cpp::Map::new();
        chat_list.append_int32(public_chat_id)?;
        chat_list.append_int32(private_chat_id)?;

        statement.bind_map(0, chat_list)?;

        match self_user_id {
            UserId::UserId(user_id) => {
                statement.bind_int32(1, *user_id)?;
            }
            UserId::Username(username) => {
                statement.bind_string(1, username)?;
            }
        }

        statement.execute().await?;

        Ok(())
    }

    pub async fn fetch_user(&self, user_id: &UserId) -> PPResult<Option<User>> {
        let statement = match user_id {
            UserId::UserId(user_id) => {
                let query = "SELECT id, name, photo, username FROM ksp.users WHERE id = ?";
                let mut statement = self.session.statement(query);
                statement.bind_int32(0, *user_id)?;
                statement
            }
            UserId::Username(username) => {
                let query = "SELECT id, name, photo, username FROM ksp.users WHERE username = ?";
                let mut statement = self.session.statement(query);
                statement.bind_string(0, username)?;
                statement
            }
        };
        let result = statement.execute().await?;

        let row = result.first_row();
        if let Some(row) = row {
            let user_id: i32 = row.get(0)?;
            let name: String = row.get(1)?;
            let photo: String = row.get(2)?;
            let username: String = row.get(3)?;

            return Ok(Some(User::construct(
                name,
                user_id,
                username,
                if photo.is_empty() { None } else { Some(photo) },
            )));
        }

        Ok(None)
    }
}
