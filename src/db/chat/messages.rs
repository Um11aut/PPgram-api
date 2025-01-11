use futures::TryStreamExt;
use log::debug;
use scylla::DeserializeRow;
use scylla::SerializeRow;

use crate::db::bucket::DatabaseBuilder;
use crate::db::init::Database;
use crate::db::internal::error::PPError;
use crate::db::internal::error::PPResult;
use crate::db::internal::validate::validate_range;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::message::Message;
use crate::server::message::types::request::send::*;
use crate::server::message::types::user::UserId;
use core::range::RangeInclusive;
use std::ops::Range;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const AS_LAST_MESSAGE_IDX: i32 = -1;

pub struct MessagesDB {
    session: Arc<scylla::Session>,
}

impl Database for MessagesDB {
    fn new(session: Arc<scylla::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS ksp.messages (
                id int,
                is_unread boolean,
                from_id int,
                chat_id int,
                edited boolean,
                date bigint,
                has_reply boolean,
                reply_to int,
                has_content boolean,
                content TEXT,
                has_hashes boolean,
                sha256_hashes LIST<TEXT>,
                PRIMARY KEY (chat_id, id)
            ) WITH CLUSTERING ORDER BY (id DESC);
        "#;

        self.session.query_unpaged(create_table_query, &[]).await?;

        self.session
            .query_unpaged(
                r#"
                    CREATE INDEX IF NOT EXISTS unread_index ON ksp.messages (is_unread);
                "#,
                &[],
            )
            .await?;

        self.session
            .query_unpaged(
                r#"
                    CREATE INDEX IF NOT EXISTS idx_messages_id ON ksp.messages(id);
                "#,
                &[],
            )
            .await?;

        Ok(())
    }
}

impl From<DatabaseBuilder> for MessagesDB {
    fn from(value: DatabaseBuilder) -> Self {
        MessagesDB {
            session: value.bucket.get_connection(),
        }
    }
}

#[derive(Debug, DeserializeRow, SerializeRow, Default)]
struct DatabaseMessage {
    id: i32,
    is_unread: bool,
    from_id: i32,
    chat_id: i32,
    edited: bool,
    date: i64,
    has_reply: bool,
    reply_to: i32,
    has_content: bool,
    content: String,
    has_hashes: bool,
    sha256_hashes: Vec<String>,
}

impl MessagesDB {
    pub async fn add_message(
        &self,
        msg: &SendMessageRequest,
        sender_id: &UserId,
        target_chat_id: ChatId,
    ) -> Result<Message, PPError> {
        let insert_query = r#"
            INSERT INTO ksp.messages
                (id, is_unread, from_id, chat_id, edited, date, has_reply,
                reply_to, has_content, content,
                has_hashes, sha256_hashes)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let mut v: DatabaseMessage = Default::default();
        match self.get_latest(target_chat_id).await? {
            Some(id) => {
                v.id = id + 1;
            }
            None => {
                v.id = 0;
            }
        };
        let prepared = self.session.prepare(insert_query).await?;

        v.is_unread = true;
        match sender_id {
            UserId::UserId(user_id) => {
                v.from_id = *user_id;
            }
            UserId::Username(_) => {
                return Err(PPError::from("UserId must be user_id, not username!"))
            }
        }
        v.chat_id = target_chat_id;
        v.edited = false;
        v.date = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        v.has_reply = msg.common.reply_to.is_some();
        v.reply_to = msg.common.reply_to.unwrap_or(0);

        match &msg.content.text {
            Some(content) => {
                v.has_content = true;
                v.content = content.into();
            }
            None => {
                v.has_content = false;
                v.content = "".into();
            }
        }
        match &msg.content.sha256_hashes {
            Some(sha256_hashes) => {
                v.has_hashes = true;
                v.sha256_hashes = sha256_hashes.clone();
            }
            None => {
                v.has_hashes = false;
                v.sha256_hashes = vec![];
            }
        }

        self.session.execute_unpaged(&prepared, v).await?;

        let msg = self.fetch_messages(target_chat_id, -1..0).await?;
        Ok(msg.into_iter().next().unwrap())
    }

    pub async fn get_latest(&self, chat_id: ChatId) -> Result<Option<MessageId>, PPError> {
        let query = "SELECT id FROM ksp.messages WHERE chat_id = ? ORDER BY id DESC LIMIT 1";
        let prepared = self.session.prepare(query).await?;
        let res = self
            .session
            .execute_iter(prepared, (chat_id,))
            .await?
            .rows_stream::<(i32,)>()?
            .try_next()
            .await?
            .map(|v| v.0);

        Ok(res)
    }

    pub async fn message_exists(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<bool, PPError> {
        let query = "SELECT id FROM ksp.messages WHERE chat_id = ? AND id = ? LIMIT 1";
        let prepared = self.session.prepare(query).await?;
        let res = self
            .session
            .execute_unpaged(&prepared, (chat_id, message_id))
            .await?;

        Ok(res.is_rows())
    }

    pub async fn fetch_messages(
        &self,
        chat_id: ChatId,
        mut range: Range<MessageId>,
    ) -> PPResult<Vec<Message>> {
        if range.start == AS_LAST_MESSAGE_IDX {
            match self.get_latest(chat_id).await? {
                Some(latest) => {
                    range.start = latest;
                }
                None => {
                    range.start = 0;
                }
            }
        }
        let (start, end) = validate_range(RangeInclusive::from(range.start..=range.end))?;

        let mut iter = if end != 0 {
            let query = r#"
                SELECT id, is_unread, from_id, chat_id, edited, date, has_reply,
                    reply_to, has_content, content, has_hashes, sha256_hashes
                    FROM ksp.messages
                    WHERE chat_id = ? AND id >= ? AND id <= ?;
            "#;
            let prepared = self.session.prepare(query).await?;
            self.session
                .execute_iter(prepared, (chat_id, start, end))
                .await?
                .rows_stream::<DatabaseMessage>()?
        } else {
            let query = r#"
                SELECT id, is_unread, from_id, chat_id, edited, date, has_reply,
                    reply_to, has_content, content, has_hashes, sha256_hashes
                    FROM ksp.messages
                    WHERE chat_id = ? AND id = ?;
            "#;
            let prepared = self.session.prepare(query).await?;
            self.session
                .execute_iter(prepared, (chat_id, start))
                .await?
                .rows_stream::<DatabaseMessage>()?
        };

        let mut output: Vec<Message> = vec![];
        while let Some(msg) = iter.try_next().await? {
            output.push(Message {
                message_id: msg.id,
                is_unread: msg.is_unread,
                from_id: msg.from_id,
                chat_id: msg.chat_id,
                is_edited: msg.edited,
                date: msg.date,
                reply_to: if msg.has_reply {
                    Some(msg.reply_to)
                } else {
                    None
                },
                content: if msg.has_content {
                    Some(msg.content)
                } else {
                    None
                },
                sha256_hashes: if msg.has_hashes {
                    Some(msg.sha256_hashes)
                } else {
                    None
                },
            });
        }

        Ok(output)
    }

    pub async fn mark_as_read(&self, chat_id: ChatId, msg_ids: &[i32]) -> PPResult<()> {
        if msg_ids.is_empty() {
            return Ok(());
        }

        for msg_id in msg_ids {
            let update_query = "
                UPDATE ksp.messages
                SET is_unread = false
                WHERE chat_id = ? AND id = ?";
            let prepared = self.session.prepare(update_query).await?;
            self.session
                .execute_unpaged(&prepared, (chat_id, msg_id))
                .await?;
        }

        Ok(())
    }

    pub async fn edit_message(
        &self,
        msg_id: i32,
        chat_id: ChatId,
        new_message: Message,
    ) -> PPResult<()> {
        let update_query = r#"
            UPDATE ksp.messages
            SET is_unread = ?,
                has_content = ?,
                content = ?,
                has_reply = ?,
                reply_to = ?,
                has_hashes = ?,
                sha256_hashes = ?,
                edited = ?
            WHERE chat_id = ? AND id = ?
        "#;

        let prepared = self.session.prepare(update_query).await?;
        self.session
            .execute_unpaged(
                &prepared,
                (
                    new_message.is_unread,
                    new_message.content.is_some(),
                    new_message.content.unwrap_or_default().as_str(),
                    new_message.reply_to.is_some(),
                    new_message.reply_to.unwrap_or(0),
                    new_message.sha256_hashes.is_some(),
                    new_message.sha256_hashes,
                    true,
                    chat_id,
                    msg_id,
                ),
            )
            .await?;

        Ok(())
    }
    pub async fn delete_messages(&self, chat_id: ChatId, message_ids: &Vec<i32>) -> PPResult<()> {
        for msg_id in message_ids {
            self.delete_message(chat_id, *msg_id).await?
        }

        Ok(())
    }

    pub async fn delete_message(&self, chat_id: ChatId, message_id: i32) -> PPResult<()> {
        let delete_query = r#"
            DELETE FROM ksp.messages
            WHERE chat_id = ? AND id = ?
        "#;

        let prepared = self.session.prepare(delete_query).await?;
        self.session
            .execute_unpaged(&prepared, (chat_id, message_id))
            .await?;

        Ok(())
    }

    /// Deletes all messages associated with a specific chat
    pub async fn delete_all_messages(&self, chat_id: ChatId) -> PPResult<()> {
        let delete_query = "DELETE FROM ksp.messages WHERE chat_id = ?";
        let prepared = self.session.prepare(delete_query).await?;
        self.session.execute_unpaged(&prepared, (chat_id,)).await?;
        Ok(())
    }

    pub async fn fetch_unread_count(&self, chat_id: ChatId) -> PPResult<Option<u64>> {
        let query = r#"
            SELECT COUNT(*)
            FROM ksp.messages
            WHERE chat_id = ? AND is_unread = true;
        "#;

        let prepared = self.session.prepare(query).await?;
        let count = self
            .session
            .execute_iter(prepared, (chat_id,))
            .await?
            .rows_stream::<(i64,)>()?
            .try_next()
            .await?
            .map(|v| v.0 as u64);

        Ok(count)
    }

    pub async fn fetch_hash_count(&self, chat_id: ChatId) -> PPResult<Option<usize>> {
        let query = r#"
            SELECT sha256_hashes
            FROM ksp.messages
            WHERE chat_id = ?;
        "#;

        let prepared = self.session.prepare(query).await?;
        let hashes = self
            .session
            .execute_iter(prepared, (chat_id,))
            .await?
            .rows_stream::<(Vec<String>,)>()?
            .try_next()
            .await?
            .map(|v| v.0.iter().map(|v| v.len()).sum::<usize>());

        Ok(hashes)
    }

    pub async fn fetch_all_hashes(&self, chat_id: ChatId) -> PPResult<Option<Vec<String>>> {
        let query = r#"
            SELECT sha256_hashes
            FROM ksp.messages
            WHERE chat_id = ?;
        "#;

        let prepared = self.session.prepare(query).await?;
        let all_hashes = self
            .session
            .execute_iter(prepared, (chat_id,))
            .await?
            .rows_stream::<(Vec<String>,)>()?
            .try_next()
            .await?
            .map(|v| v.0);

        Ok(all_hashes)
    }
}
