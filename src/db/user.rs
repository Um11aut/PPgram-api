use lazy_static::lazy_static;
use log::error;
use rand::{distributions::Alphanumeric, Rng};
use std::{str::FromStr, sync::Mutex};
use cassandra_cpp::*;
use std::sync::Arc;

lazy_static! {
    pub static ref USERS_DB: Mutex<UsersDB> = Mutex::new(UsersDB::new("127.0.0.1")); 
}

pub struct UsersDB {
    session: Arc<Session>
}

impl UsersDB {
    pub fn new(contact_points: &str) -> UsersDB {
        let mut cluster = Cluster::default();
        let cluster = cluster.set_contact_points(contact_points).expect("Failed to set contact points");

        let rt = tokio::runtime::Runtime::new().unwrap();

        let session = rt.block_on(async {
            cluster.connect().await.unwrap()
        });

        UsersDB { session: Arc::new(session) }
    }

    pub async fn create_table(&self) {
        let query = r#"
            CREATE TABLE IF NOT EXISTS users (
                id UUID PRIMARY KEY, 
                name TEXT, 
                username TEXT, 
                password_hash TEXT, 
                sessions LIST<TEXT>
            )
        "#;
        
        match self.session.execute(&query).await {
            Ok(_) => {},
            Err(err) => {
                error!("{}", err);
            },
        }
    }

    pub async fn register(&self, name: &str, username: &str, password_hash: &str) {
        let user_id = UuidGen::new_with_node(0);
        let query = r#"
            INSERT INTO users (id, name, username, password_hash, sessions) VALUES (?, ?, ?, ?, ?)
        "#;
        let mut statement = self.session.statement(query);

        statement.bind_uuid(0, user_id.gen_random()).unwrap();
        statement.bind_string(1, name).unwrap();
        statement.bind_string(2, username).unwrap();
        statement.bind_string(3, password_hash).unwrap();
        statement.bind_list(4, List::new()).unwrap();

        match statement.execute().await {
            Ok(_) => {},
            Err(err) => {
                error!("{}", err);
            },
        }
    }

    pub async fn create_session(&self, user_id: u64) -> Option<String> {
        let new_session: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        
        let query = "SELECT sessions FROM users WHERE id = ?";
        let mut statement = self.session.statement(query);
        statement.bind_uuid(0, Uuid::from_str(user_id.to_string().as_str()).unwrap()).unwrap();

        None
    }
}