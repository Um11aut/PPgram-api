use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::List;
use core::range::RangeInclusive;
use std::ops::Range;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::OnceCell;

use crate::db::db::Database;
use crate::db::internal::error::PPError;
use crate::db::internal::validate::validate_range;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::message::Message;
use crate::server::message::types::request::message::*;
use crate::server::message::types::user::UserId;


pub static MESSAGES_DB: OnceCell<MessagesDB> = OnceCell::const_new();

pub struct MessagesDB {
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
                media_hashes LIST<TEXT>,
                media_names LIST<TEXT>,
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
        sender_id: &UserId,
        target_chat_id: ChatId
    ) -> Result<Message, PPError> {
        let insert_query = r#"
            INSERT INTO messages 
                (id, is_unread, from_id, chat_id, date, has_reply,
                reply_to, has_content, content, 
                has_media, media_datas, media_names)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let mut statement = self.session.statement(&insert_query);

        match self.get_latest(target_chat_id).await? {
            Some(id) => {
                statement.bind_int32(0, id + 1)?; // id
            }
            None => {
                statement.bind_int32(0, 0)?; // id
            }
        }

        statement.bind_bool(1, true)?; // is_unread
        match sender_id {
            UserId::UserId(user_id) => {
                statement.bind_int32(2, *user_id)?; // from_id
            }
            UserId::Username(_) => {
                return Err(PPError::from("UserId must be user_id, not username!"))
            }
        }
        statement.bind_int32(3, target_chat_id)?; // chat_id
        statement.bind_int64(
            4,  // date
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        )?;
        statement.bind_bool(5, msg.common.has_reply)?; // has_reply
        statement.bind_int32(6, msg.common.reply_to)?; // reply_to
        match &msg.content {
            MessageContent::Media(media) => {
                match &media.caption {
                    Some(caption) => {
                        statement.bind_bool(7, true)?; // has_content
                        statement.bind_string(8, &caption)?; // content
                    },
                    None => {
                        statement.bind_bool(7, true)?; // has_content
                        statement.bind_string(8, "")?; // content
                    },
                }

                statement.bind_bool(9, true)?; // has_media

                statement.bind_list(10, List::new())?; // media_datas
                statement.bind_list(11, List::new())?; // media_names

                // TODO: Implement media messages
                todo!()
            }
            MessageContent::Text(text) => {
                statement.bind_bool(7, true)?; // has_content
                statement.bind_string(8, &text.text)?; // content
                statement.bind_bool(9, false)?; // has_media
                statement.bind_list(10, List::new())?; // media_datas
                statement.bind_list(11, List::new())?; // media_names
            }
        }

        statement.execute().await?;

        let msg = self.fetch_messages(target_chat_id, -1..0).await?.unwrap();
        Ok(msg.into_iter().next().unwrap())
    }

    pub async fn get_latest(&self, chat_id: ChatId) -> Result<Option<MessageId>, PPError> {
        let query = "SELECT id FROM messages WHERE chat_id = ? ORDER BY id DESC LIMIT 1";

        let mut statement = self.session.statement(&query);
        statement.bind_int32(0, chat_id)?;

        let result = statement.execute().await?;

        if let Some(row) = result.first_row() {
            let id: MessageId = row.get_column(0)?.get_i32()?;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    pub async fn message_exists(&self, chat_id: ChatId, message_id: MessageId) -> Result<bool, PPError> {
        let query = "SELECT id FROM messages WHERE chat_id = ? AND id = ? LIMIT 1";

        let mut statement = self.session.statement(query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_int32(1, message_id)?;

        let result = statement.execute().await?;

        if result.first_row().is_some() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn fetch_messages(&self, chat_id: ChatId, mut range: Range<MessageId>) -> Result<Option<Vec<Message>>, PPError> {
        if range.start == -1 {
            match self.get_latest(chat_id).await? {
                Some(latest) => {
                    range.start = latest;
                },
                None => {
                    range.start = 0;
                },
            }
        }
        let (start, end) = validate_range(RangeInclusive::from(range.start..=range.end))?;

        let statement = if end != 0 {
            let query = r#"
                SELECT * 
                    FROM messages 
                    WHERE chat_id = ? AND id >= ? AND id <= ? 
                    ORDER BY id ASC;
            "#;
            let mut statement = self.session.statement(query);
            statement.bind_int32(0, chat_id)?;
            statement.bind_int32(1, start)?;
            statement.bind_int32(2, end)?;
            statement
        } else {
            let query = r#"
                SELECT * 
                    FROM messages 
                    WHERE chat_id = ? AND id = ?;
            "#;
            let mut statement = self.session.statement(query);
            statement.bind_int32(0, chat_id)?;
            statement.bind_int32(1, start)?;
            statement
        };

        let result = statement.execute().await?;

        let mut output: Vec<Message> = vec![];
        let mut iter = result.iter();
        while let Some(row) = iter.next() {
            let message_id: i32 = row.get_by_name("id")?;
            let is_unread: bool = row.get_by_name("is_unread")?;
            let from_id: i32 = row.get_by_name("from_id")?;
            let chat_id: i32 = row.get_by_name("chat_id")?;
            let date: i64 = row.get_by_name("date")?;
            let has_reply: bool = row.get_by_name("has_reply")?;
            let reply_to: i32 = row.get_by_name("reply_to")?;
            let has_content: bool = row.get_by_name("has_content")?;
            let content: String = row.get_by_name("content")?;
            let _: bool = row.get_by_name("has_media")?;
            // TODO: Media

            output.push(
                Message{
                    message_id,
                    is_unread,
                    from_id,
                    chat_id,
                    date,
                    reply_to: if has_reply {Some(reply_to)} else {None},
                    content: if has_content && !content.is_empty() {Some(content)} else {None},
                    media_hashes: vec![],
                    media_names: vec![],
                }
            )
        }
        
        Ok(Some(output))
    }
}
