use std::sync::Arc;

use crate::{
    db::{
        bucket::DatabaseBuilder,
        db::Database,
        internal::error::{PPError, PPResult},
    },
    server::message::types::{chat::ChatId, user::UserId},
};

pub struct DraftsDB {
    session: Arc<cassandra_cpp::Session>,
}

impl Database for DraftsDB {
    fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS ksp.drafts (
                user_id int,
                chat_id int,
                content TEXT,
                PRIMARY KEY (user_id, chat_id)
            );
        "#;

        self.session.execute(create_table_query).await?;
        Ok(())
    }
}

impl DraftsDB {
    pub async fn update_draft(
        &self,
        from_user_id: &UserId,
        target_chat_id: ChatId,
        content: &str,
    ) -> PPResult<()> {
        let query = "
            BEGIN BATCH
            INSERT INTO ksp.drafts (user_id, chat_id, content) VALUES (?, ?, ?) IF NOT EXISTS;
            UPDATE ksp.drafts SET content = ? WHERE user_id = ? AND chat_id = ?;
            APPLY BATCH;
        ";
        let mut statement = self.session.statement(query);
        statement.bind_int32(0, from_user_id.as_i32_unchecked())?;
        statement.bind_int32(1, target_chat_id)?;
        statement.bind_string(2, content)?;
        statement.bind_string(3, content)?;
        statement.bind_int32(4, from_user_id.as_i32_unchecked())?;
        statement.bind_int32(5, target_chat_id)?;

        Ok(())
    }
}

impl From<DatabaseBuilder> for DraftsDB {
    fn from(value: DatabaseBuilder) -> Self {
        Self {
            session: value.bucket.get_connection(),
        }
    }
}
