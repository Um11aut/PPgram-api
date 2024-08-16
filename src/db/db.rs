use std::sync::Arc;

use tokio::sync::OnceCell;

use log::error;

use super::{chat::{chats::CHATS_DB, messages::MESSAGES_DB}, internal::error::DatabaseError, user::USERS_DB};

pub(crate) trait Database {
    async fn new(session: Arc<cassandra_cpp::Session>) -> Self;
    async fn create_table(&self) -> Result<(), DatabaseError>;
}

async fn init<T: Database>(db: &OnceCell<T>, session: Arc<cassandra_cpp::Session>) {
    db.get_or_init(|| async { T::new(Arc::clone(&session)).await }).await;
    db.get().unwrap().create_table().await.unwrap();
}

async fn create_connection() -> Arc<cassandra_cpp::Session> {
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

    Arc::new(session)
}

pub async fn init_dbs() {
    init(&USERS_DB, create_connection().await).await;
    init(&CHATS_DB, create_connection().await).await;
    init(&MESSAGES_DB, create_connection().await).await;
}
