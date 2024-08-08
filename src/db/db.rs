use std::sync::Arc;

use log::error;

use super::{
    messages::{MessagesDB, MESSAGES_DB},
    user::{UsersDB, USERS_DB},
};

pub(crate) trait Database {
    async fn new(session: Arc<cassandra_cpp::Session>) -> Self;
    async fn create_table(&self);
}

pub async fn init_dbs() {
    let contact_points = std::env::var("CASSANDRA_HOST").unwrap_or(String::from("127.0.0.1"));

    let mut cluster = cassandra_cpp::Cluster::default();
    let cluster = cluster
        .set_contact_points(contact_points.as_str())
        .expect("Failed to set contact points");
    cluster.set_load_balance_round_robin();

    while let Err(err) = cluster.connect().await {
        error!("{}", err);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let session = cluster.connect().await.unwrap();

    session
        .execute(
            "
            CREATE KEYSPACE IF NOT EXISTS main_keyspace
            WITH REPLICATION = { 'class': 'SimpleStrategy', 'replication_factor': 1 };
        ",
        )
        .await
        .unwrap();
    session.execute("USE main_keyspace").await.unwrap();

    let session = Arc::new(session);
    USERS_DB
        .get_or_init(|| async { UsersDB::new(Arc::clone(&session)).await })
        .await;

    MESSAGES_DB
        .get_or_init(|| async { MessagesDB::new(Arc::clone(&session)).await })
        .await;

    MESSAGES_DB.get().unwrap();
    USERS_DB.get().unwrap().create_table().await;
}
