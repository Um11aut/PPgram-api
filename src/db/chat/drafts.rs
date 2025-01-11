use std::sync::Arc;

use futures::TryStreamExt;

use crate::{
    db::{
        bucket::DatabaseBuilder,
        init::Database,
        internal::error::{PPError, PPResult},
    },
    server::message::types::{chat::ChatId, user::UserId},
};

pub struct DraftsDB {
    session: Arc<scylla::Session>,
}

impl Database for DraftsDB {
    fn new(session: Arc<scylla::Session>) -> Self {
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

        self.session.query_unpaged(create_table_query, &[]).await?;
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
        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(
                &prepared,
                (
                    from_user_id.as_i32_unchecked(),
                    target_chat_id,
                    content.to_owned(),
                    content.to_owned(),
                    from_user_id.as_i32_unchecked(),
                    target_chat_id,
                ),
            )
            .await?;

        Ok(())
    }

    pub async fn fetch_draft(
        &self,
        self_user_id: &UserId,
        chat_id: ChatId,
    ) -> PPResult<Option<String>> {
        let query = "SELECT content FROM ksp.drafts WHERE user_id = ? AND chat_id = ?";
        let prepared = self.session.prepare(query).await?;

        Ok(self
            .session
            .execute_iter(prepared, (self_user_id.as_i32_unchecked(), chat_id))
            .await?
            .rows_stream::<(String,)>()?
            .try_next()
            .await?
            .map(|v| v.0))
    }
}

impl From<DatabaseBuilder> for DraftsDB {
    fn from(value: DatabaseBuilder) -> Self {
        Self {
            session: value.bucket.get_connection(),
        }
    }
}
