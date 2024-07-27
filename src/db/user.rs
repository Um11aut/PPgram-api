use lazy_static::lazy_static;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::str::FromStr;
use tokio::sync::Mutex;
use cassandra_cpp::*;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub(crate) static USERS_DB: OnceCell<UsersDB> = OnceCell::const_new();

pub(crate) struct UsersDB {
    session: Arc<Session>
}

impl UsersDB {
    pub async fn new(contact_points: &str) -> UsersDB {
        let mut cluster = Cluster::default();
        let cluster = cluster.set_contact_points(contact_points).expect("Failed to set contact points");
        
        let session = cluster.connect().await.unwrap();
        session.execute("USE usersdb_keyspace").await.unwrap();
        UsersDB { session: Arc::new(session) }
    }

    pub async fn create_table(&self) {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS users (
                id int PRIMARY KEY, 
                name TEXT, 
                username TEXT, 
                password_hash TEXT, 
                sessions SET<TEXT>
            )
        "#;
        
        let create_index_query = r#"
            CREATE INDEX IF NOT EXISTS username_idx ON users (username)
        "#;

        match self.session.execute(create_table_query).await {
            Ok(_) => {},
            Err(err) => {
                error!("{}", err);
            },
        }

        match self.session.execute(create_index_query).await {
            Ok(_) => {},
            Err(err) => {
                error!("{}", err);
            },
    }
    }

    pub async fn register(
            &self, 
            name: &str, 
            username: &str, 
            password_hash: &str
        ) -> std::result::Result<(), Error> {
        let query = "SELECT id FROM users WHERE username = ?";
        let mut statement = self.session.statement(query);
        statement.bind_string(0, username).unwrap();
    
        let user_exists: bool = match statement.execute().await {
            Ok(result) => result.first_row().is_some(),
            Err(err) => {
                return Err(err);
            },
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
        statement.bind_set(4, Set::new()).unwrap();

        statement.execute().await?;

        self.create_session(user_id).await;

        Ok(())
    }

    pub async fn create_session(&self, user_id: i32) -> Option<String> {
        let new_session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        
        let query = "SELECT sessions FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, user_id).unwrap();

        let existing_sessions: Option<Vec<String>> = match statement.execute().await {
            Ok(result) => {
                let row= result.first_row().unwrap();
                let value: Result<String, > = row.get(0);
                info!("{}", value.unwrap());

                None
            },
            Err(err) => {
                error!("[::create_session] {}", err);
                return None;
            },
        };

        None
    }
}

pub async fn init_db()
{
    USERS_DB.get_or_init(|| async {
        UsersDB::new("127.0.0.1").await
    }).await;

    USERS_DB.get().unwrap().create_table().await;

}