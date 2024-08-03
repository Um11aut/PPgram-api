
use cassandra_cpp::*;
use lazy_static::lazy_static;
use log::{debug, error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

use crate::server;

pub(crate) static USERS_DB: OnceCell<UsersDB> = OnceCell::const_new();

pub(crate) struct UsersDB {
    session: Arc<Session>,
}

impl UsersDB {
    pub async fn new(contact_points: &str) -> UsersDB {
        let mut cluster = Cluster::default();
        let cluster = cluster
            .set_contact_points(contact_points)
            .expect("Failed to set contact points");
        cluster.set_load_balance_round_robin();

        let session = cluster.connect().await.unwrap();
        session.execute("USE usersdb_keyspace").await.unwrap();
        UsersDB {
            session: Arc::new(session),
        }
    }

    pub async fn create_table(&self) {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS users (
                id int PRIMARY KEY, 
                name TEXT, 
                username TEXT, 
                password_hash TEXT, 
                sessions LIST<TEXT>
            )
        "#;

        let create_index_query = r#"
            CREATE INDEX IF NOT EXISTS username_idx ON users (username)
        "#;

        match self.session.execute(create_table_query).await {
            Ok(_) => {}
            Err(err) => {
                error!("{}", err);
            }
        }

        match self.session.execute(create_index_query).await {
            Ok(_) => {}
            Err(err) => {
                error!("{}", err);
            }
        }
    }

    // Register the user in database. Returns `user_id` and `session_id` if successfull
    pub async fn register(
        &self,
        name: &str,
        username: &str,
        password_hash: &str,
    ) -> std::result::Result<(i32 /* user_id */, String /* session_id */), Error> {
        let query = "SELECT id FROM users WHERE username = ?";
        let mut statement = self.session.statement(query);
        statement.bind_string(0, username).unwrap();

        let user_exists: bool = match statement.execute().await {
            Ok(result) => result.first_row().is_some(),
            Err(err) => {
                return Err(err);
            }
        };

        if user_exists {
            return Err(Error::from("Username already taken"));
        }

        let user_id = rand::random::<i32>();
        let query = r#"
            INSERT INTO users (id, name, username, password_hash, sessions) VALUES (?, ?, ?, ?, ?)
        "#;
        let mut statement = self.session.statement(query);

        statement.bind_int32(0, user_id).unwrap();
        statement.bind_string(1, name).unwrap();
        statement.bind_string(2, username).unwrap();
        statement.bind_string(3, password_hash).unwrap();
        statement.bind_list(4, List::new()).unwrap();

        statement.execute().await?;

        match self.create_session(user_id).await {
            Ok(session_id) => return Ok((user_id, session_id)),
            Err(err) => return Err(err),
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password_hash: &str,
    ) -> std::result::Result<(i32 /* user_id */, String /* session_id */), Error> {
        let query = "SELECT id, password_hash FROM users WHERE username = ?";
        let mut statement = self.session.statement(query);
        statement.bind_string(0, username).unwrap();

        let (user_id, stored_password_hash): (Option<i32>, Option<String>) =
            match statement.execute().await {
                Ok(result) => {
                    if let Some(row) = result.first_row() {
                        let user_id: i32 = row.get(0).unwrap_or_default();
                        let stored_password_hash: String = row.get(1).unwrap_or_default();
                        (Some(user_id), Some(stored_password_hash))
                    } else {
                        (None, None)
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            };

        if let (Some(user_id), Some(stored_password_hash)) = (user_id, stored_password_hash) {
            if stored_password_hash != password_hash {
                return Err(Error::from("Invalid password"));
            }

            match self.create_session(user_id).await {
                Ok(session_id) => return Ok((user_id, session_id)),
                Err(err) => return Err(err),
            }
        } else {
            return Err(Error::from("User with the given credentials not found!"));
        }
    }


    pub async fn authenticate(&self, user_id: i32, session_id: &str, password_hash: &str) -> std::result::Result<(), Error> {

        let query = "SELECT password_hash, sessions FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id).unwrap();

        let (id, stored_password_hash, sessions): (Option<i32>, Option<String>, Option<Vec<String>>) =
            match statement.execute().await {
                Ok(result) => {
                    if let Some(row) = result.first_row() {
                        let stored_password_hash: String = row.get(0).unwrap_or_default();
                        let result: Result<SetIterator, > = row.get(1);

                        let mut o: Vec<String> = Vec::with_capacity(3);
                        if let Ok(mut sessions) = result {
                            while let Some(session) = sessions.next() {
                                o.push_within_capacity(session.to_string()).unwrap();
                            }
                        }

                        (Some(user_id), Some(stored_password_hash), Some(o))
                    } else {
                        (None, None, None)
                    }
                }
                Err(err) => {
                    error!("{}", err);
                    return Err(err);
                }
            };

        if let (Some(id), Some(stored_password_hash), Some(sessions)) = (id, stored_password_hash, sessions) {
            if stored_password_hash != password_hash {
                return Err(Error::from("Invalid password"));
            }

            if id != user_id {
                return Err(Error::from("User Id wasn't found!"));
            }

            if sessions.iter().find(|&s| s == session_id).is_none() {
                return Err(Error::from("Your Session is expired!"));
            }

            Ok(())
        } else {
            return Err(Error::from("User with the given credentials not found!"));
        }
    }

    async fn create_session(&self, user_id: i32) -> std::result::Result<String, Error> {
        let new_session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let query = "SELECT sessions FROM users WHERE id = ?;";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id).unwrap();

        let mut existing_sessions: Vec<String> = match statement.execute().await {
            Ok(result) => {
                let mut o = Vec::with_capacity(3);

                let mut iter = result.iter();
                while let Some(row) = iter.next() {
                    let res: Result<SetIterator> = row.get(0);

                    if let Ok(mut sessions) = res {
                        while let Some(session) = sessions.next() {
                            o.push_within_capacity(session.to_string()).unwrap();
                        }
                    }
                }

                o
            }
            Err(err) => {
                error!("[::create_session] {}", err);
                return Err(Error::from("Internal error."));
            }
        };

        // If session exceed the maximum size, delete the first one
        if existing_sessions.len() == 3 {
            existing_sessions.drain(..1);
        }

        let query = "UPDATE users SET sessions = ? WHERE id = ?";
        let mut statement = self.session.statement(query);

        let mut updated_sessions = List::new();
        for session in existing_sessions {
            updated_sessions.append_string(session.as_str()).unwrap();
        }

        updated_sessions
            .append_string(new_session.as_str())
            .unwrap();

        statement.bind_list(0, updated_sessions).unwrap();
        statement.bind_int32(1, user_id).unwrap();

        match statement.execute().await {
            Ok(_) => Ok(new_session),
            Err(err) => {
                error!("[::create_session] {}", err);
                return Err(Error::from("Internal error."));
            }
        }
    }
}

pub async fn init_db() {
    USERS_DB
        .get_or_init(|| async { UsersDB::new("127.0.0.1").await })
        .await;

    USERS_DB.get().unwrap().create_table().await;
}
