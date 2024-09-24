use std::sync::Arc;

use tokio::sync::OnceCell;

use log::error;

use super::{chat::{chats::ChatsDB, messages::MessagesDB}, connection::{DatabaseBuilder, DatabasePool}, internal::error::PPError, user::UsersDB};

pub trait Database {
    fn new(session: Arc<cassandra_cpp::Session>) -> Self; 
    fn create_table(&self) -> impl std::future::Future<Output = Result<(), PPError>> + Send;
}

/// Creates a temporary database pool for creating basic tables
/// Deallocating pool at the end
pub async fn create_tables() {
    let mut pool: DatabasePool = DatabasePool::new().await;
    let bucket = pool.get_available_bucket().await;
    let users_db: UsersDB = DatabaseBuilder::from(bucket.clone()).into();
    let messages_db: MessagesDB = DatabaseBuilder::from(bucket.clone()).into();
    let chats_db: ChatsDB = DatabaseBuilder::from(bucket.clone()).into();

    users_db.create_table().await;
    messages_db.create_table().await;
    chats_db.create_table().await;
}