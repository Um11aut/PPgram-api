use argon2::{
    password_hash::{
        self, rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use futures::{stream::iter, StreamExt, TryStreamExt};
use log::{debug, error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::User;
use crate::server::message::types::user::UserId;

use super::db::Database;
use super::internal::error::PPError;
use super::internal::error::PPResult;
use super::internal::validate;
use super::{bucket::DatabaseBuilder, chat::hashes::HashesDB};

pub struct UsersDB {
    session: Arc<scylla::Session>,
}

impl From<DatabaseBuilder> for UsersDB {
    fn from(value: DatabaseBuilder) -> Self {
        UsersDB {
            session: value.bucket.get_connection(),
        }
    }
}

impl Database for UsersDB {
    fn new(session: Arc<scylla::Session>) -> UsersDB {
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
            CREATE INDEX IF NOT EXISTS name_sai_index ON ksp.users (name);
        "#;

        let name_custom_index_query = r#"
            CREATE INDEX IF NOT EXISTS username_sai_index ON ksp.users (username);
        "#;

        self.session.query_unpaged(create_table_query, &[]).await?;
        self.session.query_unpaged(create_index_query, &[]).await?;
        self.session
            .query_unpaged(username_custom_index_query, &[])
            .await?;
        self.session
            .query_unpaged(name_custom_index_query, &[])
            .await?;

        Ok(())
    }
}

impl UsersDB {
    pub async fn exists(&self, identifier: &UserId) -> PPResult<bool> {
        let result = match identifier {
            UserId::UserId(user_id) => {
                let query = "SELECT id FROM ksp.users WHERE id = ?";
                let prepared = self.session.prepare(query).await?;
                self.session
                    .execute_iter(prepared, (*user_id,))
                    .await?
                    .rows_stream::<(i32,)>()?
                    .try_next()
                    .await?
                    .map(|v| v.0)
            }
            UserId::Username(username) => {
                let query = "SELECT id FROM ksp.users WHERE username = ?";
                let prepared = self.session.prepare(query).await?;
                self.session
                    .execute_iter(prepared, (username,))
                    .await?
                    .rows_stream::<(i32,)>()?
                    .try_next()
                    .await?
                    .map(|v| v.0)
            }
        };

        Ok(result.is_some())
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
        let prepared = self.session.prepare(query).await?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|err| PPError::from(format!("Failed to hash password: {}", err)))?
            .to_string();
        info!("Generated new password hash: {}", password_hash);

        self.session
            .execute_unpaged(
                &prepared,
                (
                    user_id,
                    name,
                    username,
                    password_hash,
                    salt.as_str(),
                    Vec::<String>::new(),
                    "",
                    HashMap::<i32, i32>::new(),
                ),
            )
            .await?;

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
        let mut search_query: String = format!("%{}%", search_query);

        let scylla_query = match search_query.contains("@") {
            true => {
                if search_query.len() < 2 {
                    return Ok(vec![]);
                }
                // Drain '@' symbol
                search_query.remove(1);
                "SELECT username, id, photo, name FROM ksp.users WHERE username LIKE ? LIMIT 50 ALLOW FILTERING;"
            }
            false => "SELECT username, id, photo, name FROM ksp.users WHERE name LIKE ? LIMIT 50 ALLOW FILTERING;",
        };
        let mut rows_stream = self
            .session
            .query_iter(scylla_query, (search_query,))
            .await?
            .rows_stream::<(String, i32, String, String)>()?;

        let mut o = vec![];
        while let Some((username, user_id, photo, name)) = rows_stream.try_next().await? {
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
        let mut res = self
            .session
            .query_iter(query, (username,))
            .await?
            .rows_stream::<(i32, String)>()?;

        let (user_id, stored_password_hash) = res
            .try_next()
            .await?
            .ok_or(PPError::from("User with the given credentials not found!"))?;

        let password_matches = Argon2::default()
            .verify_password(
                password.as_bytes(),
                &PasswordHash::new(&stored_password_hash).expect("Failed to parse password hash!"),
            )
            .is_ok();
        if !password_matches {
            return Err(PPError::from("Invalid password!"));
        }

        match self.create_session(user_id).await {
            Ok(session_id) => Ok((user_id, session_id)),
            Err(err) => Err(err),
        }
    }

    pub async fn authenticate(&self, user_id: i32, session_id: &str) -> PPResult<()> {
        let query = "SELECT sessions FROM ksp.users WHERE id = ?";
        let (sessions,) = self
            .session
            .query_iter(query, (user_id,))
            .await?
            .rows_stream::<(Vec<String>,)>()?
            .try_next()
            .await?
            .ok_or(PPError::from("User wasn't found!"))?;

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
        let (mut sessions,) = self
            .session
            .query_iter(query, (user_id,))
            .await?
            .rows_stream::<(Vec<String>,)>()?
            .try_next()
            .await?
            .ok_or(PPError::from("User wasn't found!"))?;

        // If sessions array exceeds the maximum size, delete the first one
        if sessions.len() == 3 {
            sessions.drain(..1);
        }
        sessions.push(new_session.to_owned());

        let query = "UPDATE ksp.users SET sessions = ? WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(&prepared, (sessions, user_id))
            .await?;

        Ok(new_session)
    }

    pub async fn update_photo(&self, self_user_id: &UserId, media_hash: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET photo = ? WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(&prepared, (media_hash, self_user_id.as_i32_unchecked()))
            .await?;

        Ok(())
    }

    pub async fn update_name(&self, self_user_id: &UserId, name: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET name = ? WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(&prepared, (name, self_user_id.as_i32_unchecked()))
            .await?;

        Ok(())
    }

    pub async fn update_username(&self, self_user_id: &UserId, username: &str) -> PPResult<()> {
        let query = "UPDATE ksp.users SET username = ? WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(&prepared, (username, self_user_id.as_i32_unchecked()))
            .await?;

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

        let query = "UPDATE ksp.users SET password_hash = ?, password_salt = ? WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(
                &prepared,
                (
                    password_hash,
                    salt.as_str(),
                    self_user_id.as_i32_unchecked(),
                ),
            )
            .await?;

        Ok(())
    }

    /// fetches every chat by given user_id
    ///
    /// where i32 is the UserId
    pub async fn fetch_chats(&self, user_id: &UserId) -> PPResult<HashMap<i32, ChatId>> {
        let query = "SELECT chats FROM ksp.users WHERE id = ?";
        let mut iter = self
            .session
            .query_iter(query, (user_id.as_i32_unchecked(),))
            .await?
            .rows_stream::<(HashMap<i32, ChatId>,)>()?;

        let mut o = HashMap::new();
        while let Some((chats,)) = iter.try_next().await? {
            o.extend(chats);
        }

        Ok(o)
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
        let query = "SELECT chats FROM ksp.users WHERE id = ?";
        let maybe = self
            .session
            .query_iter(query, (self_user_id.as_i32_unchecked(),))
            .await?
            .rows_stream::<(HashMap<i32, i32>,)>()?
            .try_next()
            .await?;
        let chat_id = maybe.and_then(|(map,)| {
            map.get(&key_chat_id).copied()
        });

        Ok(chat_id)
    }

    /// Adds to a `chats` map new `Key, Value`
    ///
    /// Key is public chat id that is relative and visible to the self user
    ///
    /// `private_chat_id` is the real chat id.
    pub async fn add_associated_chat(
        &self,
        self_user_id: &UserId,
        public_chat_id: ChatId,
        private_chat_id: ChatId,
    ) -> PPResult<()> {
        let mut map = HashMap::new();
        map.insert(public_chat_id, private_chat_id);

        match self_user_id {
            UserId::UserId(user_id) => {
                let query = "UPDATE ksp.users SET chats = chats + ? WHERE id = ?";
                let prepared = self.session.prepare(query).await?;
                self.session
                    .execute_unpaged(&prepared, (map, *user_id))
                    .await?;
            }
            UserId::Username(username) => {
                let query = "UPDATE ksp.users SET chats = chats + ? WHERE username = ?";
                let prepared = self.session.prepare(query).await?;
                self.session
                    .execute_unpaged(&prepared, (map, username))
                    .await?;
            }
        };

        Ok(())
    }

    pub async fn fetch_user(&self, user_id: &UserId) -> PPResult<Option<User>> {
        Ok(match user_id {
            UserId::UserId(user_id) => {
                let query = "SELECT id, name, photo, username FROM ksp.users WHERE id = ?";
                let maybe = self
                    .session
                    .query_iter(query, (*user_id,))
                    .await?
                    .rows_stream::<(i32, String, String, String)>()?
                    .try_next()
                    .await?;
                maybe.map(|(user_id, name, photo, username)| {
                    User::construct(
                        name,
                        user_id,
                        username,
                        if photo.is_empty() { None } else { Some(photo) },
                    )
                })
            }
            UserId::Username(username) => {
                let query = "SELECT id, name, photo, username FROM ksp.users WHERE username = ?";
                let maybe = self
                    .session
                    .query_iter(query, (username,))
                    .await?
                    .rows_stream::<(i32, String, String, String)>()?
                    .try_next()
                    .await?;
                maybe.map(|(user_id, name, photo, username)| {
                    User::construct(
                        name,
                        user_id,
                        username,
                        if photo.is_empty() { None } else { Some(photo) },
                    )
                })
            }
        })
    }
}
