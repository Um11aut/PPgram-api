use futures::TryStreamExt;
use log::debug;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::sync::Arc;

use db::internal::error::PPError;

use crate::db;
use crate::db::bucket::DatabaseBuilder;
use crate::db::db::Database;
use crate::db::internal::error::PPResult;
use crate::db::user::UsersDB;
use crate::server::message::types::chat::Chat;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::UserId;

pub struct ChatsDB {
    session: Arc<scylla::Session>,
}

// to avoid confusion
pub type InvitationHash = String;

impl Database for ChatsDB {
    fn new(session: Arc<scylla::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS ksp.chats (
                id int PRIMARY KEY,
                is_group boolean,
                participants LIST<int>,

                name TEXT,
                avatar_hash TEXT,
                tag TEXT,
                invitation_hash TEXT
            );
        "#;

        self.session.query_unpaged(create_table_query, &[]).await?;
        self.session.query_unpaged("CREATE INDEX IF NOT EXISTS chats_invitation_hash_idx ON ksp.chats (invitation_hash)", &[]).await?;

        Ok(())
    }
}

impl ChatsDB {
    pub async fn create_private(
        &self,
        self_user_id: &UserId,
        participants: Vec<UserId>,
    ) -> PPResult<(Chat, ChatDetails)> {
        let chat_id = rand::thread_rng().gen_range(1..i32::MAX);
        let insert_query = "INSERT INTO ksp.chats (id, is_group, participants) VALUES (?, ?, ?)";
        let prepared = self.session.prepare(insert_query).await?;
        self.session.execute_unpaged(
            &prepared,
            (
                chat_id,
                false,
                participants
                    .iter()
                    .map(|u| u.as_i32_unchecked())
                    .collect::<Vec<i32>>()
            ),
        ).await?;

        Ok(self.fetch_chat(self_user_id, chat_id).await?.unwrap())
    }

    /// Creates new unique invitation hash for a group
    ///
    /// e.g. For others to join it
    pub async fn create_invitation_hash(&self, group_chat_id: i32) -> PPResult<InvitationHash> {
        let hash: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(14)
            .map(char::from)
            .collect();
        let new_invitation_hash = format!("+{}", hash);

        let update_query = "UPDATE ksp.chats SET invitation_hash = ? WHERE id = ?";
        let prepared = self.session.prepare(update_query).await?;
        self.session
            .execute_unpaged(&prepared, (new_invitation_hash.as_str(), group_chat_id))
            .await?;

        Ok(new_invitation_hash)
    }

    pub async fn get_chat_by_invitation_hash(
        &self,
        self_user_id: &UserId,
        invitation_hash: InvitationHash,
    ) -> PPResult<Option<(Chat, ChatDetails)>> {
        let select_query =
            "SELECT id, is_group, participants, name, avatar_hash, tag FROM ksp.chats WHERE invitation_hash = ?";

        let prepared = self.session.prepare(select_query).await?;
        let res = self
            .session
            .execute_iter(prepared, (invitation_hash,))
            .await?
            .rows_stream::<(
                i32,
                bool,
                Vec<i32>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>()?
            .try_next()
            .await?;

        if let Some((chat_id, is_group, participants, name, avatar_hash, tag)) = res {
            let users_db: UsersDB = DatabaseBuilder::from_raw(self.session.clone()).into();

            let mut users = vec![];
            for participant in participants {
                let user = users_db.fetch_user(&participant.into()).await?;
                if let Some(user) = user {
                    users.push(user)
                }
            }

            let chat = Chat::construct(chat_id, is_group, users);
            let details = if is_group {
                ChatDetails {
                    name: name.unwrap_or("".into()),
                    chat_id,
                    is_group: true,
                    photo: avatar_hash.filter(|hash| !hash.is_empty()),
                    tag: tag.filter(|tag| !tag.is_empty()),
                }
            } else {
                chat.get_personal_chat_details(self_user_id).await?
            };

            return Ok(Some((chat, details)));
        }
        Ok(None)
    }

    pub async fn create_group(
        &self,
        self_user_id: &UserId,
        participants: Vec<UserId>,
        details: ChatDetails,
    ) -> PPResult<(Chat, ChatDetails)> {
        let chat_id = rand::thread_rng().gen_range(i32::MIN..-1);
        let insert_query = "INSERT INTO ksp.chats (id, is_group, participants, name, avatar_hash, tag) VALUES (?, ?, ?, ?, ?, ?)";

        let prepared = self.session.prepare(insert_query).await?;
        self.session
            .execute_iter(
                prepared,
                (
                    chat_id,
                    true,
                    participants
                        .iter()
                        .map(|u| u.as_i32_unchecked())
                        .collect::<Vec<i32>>(),
                    details.name(),
                    details.photo().map_or("", |v| v),
                    details.tag().map_or("", |v| v),
                ),
            )
            .await?;

        Ok(self.fetch_chat(self_user_id, chat_id).await?.unwrap())
    }

    pub async fn add_participant(
        &self,
        chat_id: ChatId,
        participant: &UserId,
    ) -> Result<(), PPError> {
        let update_query = "UPDATE ksp.chats SET participants = participants + ? WHERE id = ?;";
        let prepared = self.session.prepare(update_query).await?;
        self.session.execute_unpaged(&prepared, (vec![participant.as_i32_unchecked()], chat_id)).await?;

        Ok(())
    }

    pub async fn chat_exists(&self, chat_id: ChatId) -> PPResult<bool> {
        let query = "SELECT * FROM ksp.chats WHERE id = ?";
        let prepared = self.session.prepare(query).await?;
        let res = self.session.execute_unpaged(&prepared, (chat_id,)).await?;

        Ok(res.is_rows())
    }

    /// Fetch chat by real chat id
    pub async fn fetch_chat(
        &self,
        self_user_id: &UserId,
        chat_id: ChatId,
    ) -> PPResult<Option<(Chat, ChatDetails)>> {
        let select_query =
            "SELECT id, is_group, participants, name, avatar_hash, tag FROM ksp.chats WHERE id = ?";

        let prepared = self.session.prepare(select_query).await?;
        let res = self
            .session
            .execute_iter(prepared, (chat_id,))
            .await?
            .rows_stream::<(
                i32,
                bool,
                Vec<i32>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>()?
            .try_next()
            .await?;

        if let Some((chat_id, is_group, participants, name, avatar_hash, tag)) = res {
            // can be bitcasted, because UsersDB and ChatsDB are actually the same
            let users_db: UsersDB = unsafe { std::mem::transmute(self.session.clone()) };

            let mut users = vec![];

            for participant in participants {
                let user = users_db.fetch_user(&participant.into()).await?;
                if let Some(user) = user {
                    users.push(user)
                }
            }

            let chat = Chat::construct(chat_id, is_group, users);
            let details = if is_group {
                ChatDetails {
                    name: name.unwrap_or("".into()),
                    chat_id,
                    is_group: true,
                    photo: avatar_hash.filter(|hash| !hash.is_empty()),
                    tag: tag.filter(|tag| !tag.is_empty()),
                }
            } else {
                chat.get_personal_chat_details(self_user_id).await?
            };

            return Ok(Some((chat, details)));
        }

        Ok(None)
    }
}

impl From<DatabaseBuilder> for ChatsDB {
    fn from(value: DatabaseBuilder) -> Self {
        ChatsDB {
            session: value.bucket.get_connection(),
        }
    }
}
