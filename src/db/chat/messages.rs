use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use log::{error, info};
use rand::{distributions::Alphanumeric, Rng};
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::OnceCell;

use crate::db::db::Database;
use crate::db::internal::error::PPError;
use crate::server::message::types::message::MessageContent;
use crate::server::message::types::message::RequestMessage;

pub(crate) static MESSAGES_DB: OnceCell<MessagesDB> = OnceCell::const_new();

pub(crate) struct MessagesDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for MessagesDB {
    async fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS messages (
                id int, 
                is_unread boolean,
                from_id int,
                chat_id int,
                date bigint,
                has_reply boolean,
                reply_to int,
                has_content boolean,
                content TEXT,
                has_media boolean,
                media_data BLOB,
                media_type TEXT,
                PRIMARY KEY (chat_id, id)
            )
        "#;

        self.session.execute(create_table_query).await?;

        Ok(())
    }
}

impl MessagesDB {
    pub async fn add_message(
        &self,
        msg: &RequestMessage,
        sender_id: i32,
        target_chat_id: i32
    ) -> Result<(), PPError> {
        let insert_query = r#"
            INSERT INTO messages 
                (id, is_unread, from_id, chat_id, date, has_reply,
                reply_to, has_content, content, 
                has_media, media_data, media_type)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let mut statement = self.session.statement(&insert_query);

        match self.get_latest(target_chat_id).await? {
            Some(id) => {
                statement.bind_int32(0, id + 1)?;
            }
            None => {
                statement.bind_int32(0, 0)?;
            }
        }

        statement.bind_bool(1, true)?;
        statement.bind_int32(2, sender_id)?;
        statement.bind_int32(3, target_chat_id)?;
        statement.bind_int64(
            4,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        )?;
        statement.bind_bool(5, msg.common.has_reply)?;
        statement.bind_int32(6, msg.common.reply_to)?;
        match &msg.content {
            MessageContent::Media(media) => {
                todo!()
            }
            MessageContent::Text(text) => {
                statement.bind_bool(7, true)?;
                statement.bind_string(8, &text.text)?;
                statement.bind_bool(9, false)?;
            }
        }

        statement.execute().await?;

        Ok(())
    }

    pub async fn get_latest(&self, chat_id: i32) -> Result<Option<i32>, PPError> {
        let query = "SELECT id FROM messages WHERE chat_id = ? ORDER BY id DESC LIMIT 1";

        let mut statement = self.session.statement(&query);
        statement.bind_int32(0, chat_id)?;

        let result = statement.execute().await?;

        if let Some(row) = result.first_row() {
            let id: i32 = row.get_column(0)?.get_i32()?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
}
