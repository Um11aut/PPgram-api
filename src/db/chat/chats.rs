use cassandra_cpp;
use cassandra_cpp::AsRustType;
use cassandra_cpp::CassCollection;
use cassandra_cpp::LendingIterator;
use cassandra_cpp::SetIterator;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::sync::Arc;

use db::internal::error::PPError;

use crate::db;
use crate::db::bucket::DatabaseBucket;
use crate::db::bucket::DatabaseBuilder;
use crate::db::db::Database;
use crate::db::internal::error::PPResult;
use crate::db::user::UsersDB;
use crate::server::message::types::chat::Chat;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::chat::ChatId;
use crate::server::message::types::user::User;
use crate::server::message::types::user::UserId;

pub struct ChatsDB {
    session: Arc<cassandra_cpp::Session>,
}

// to avoid confusion
pub type InvitationHash = String;

impl Database for ChatsDB {
    fn new(session: Arc<cassandra_cpp::Session>) -> Self {
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
                username TEXT,
                invitation_hash TEXT
            );
        "#;

        self.session.execute(create_table_query).await?;
        self.session.execute("CREATE INDEX IF NOT EXISTS chats_invitation_hash_idx ON ksp.chats (invitation_hash)").await?;

        Ok(())
    }
}

impl ChatsDB {
    pub async fn create_private(&self, participants: Vec<UserId>) -> Result<Chat, PPError> {
        let chat_id = rand::thread_rng().gen_range(1..i32::MAX);
        let insert_query = "INSERT INTO ksp.chats (id, is_group, participants) VALUES (?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, false)?;

        let mut list = cassandra_cpp::List::new();
        for participant in participants {
            match participant {
                UserId::UserId(user_id) => {
                    list.append_int32(user_id)?;
                }
                UserId::Username(_) => return Err(PPError::from("Cannot add chat with username!")),
            }
        }
        statement.bind_list(2, list)?;

        statement.execute().await?;

        Ok(self.fetch_chat(chat_id).await?.unwrap())
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

        let mut statement = self.session.statement(update_query);
        statement.bind_string(0, &new_invitation_hash)?;
        statement.bind_int32(1, group_chat_id)?;

        statement.execute().await.map_err(PPError::from)?;

        Ok(new_invitation_hash)
    }

    pub async fn get_chat_by_invitation_hash(
        &self,
        invitation_hash: InvitationHash,
    ) -> PPResult<Option<Chat>> {
        let select_query = "SELECT * FROM ksp.chats WHERE invitation_hash = ?";

        let mut statement = self.session.statement(select_query);
        statement.bind_string(0, &invitation_hash)?;

        match statement.execute().await {
            Ok(result) => {
                if let Some(row) = result.first_row() {
                    let chat_id: i32 = row.get_by_name("id")?;
                    let is_group: bool = row.get_by_name("is_group")?;

                    let mut iter: SetIterator = row.get_by_name("participants")?;
                    let users_db: UsersDB = DatabaseBuilder::from_raw(self.session.clone()).into();

                    let mut participants: Vec<User> = vec![];
                    while let Some(participant) = iter.next() {
                        let user = users_db.fetch_user(&participant.get_i32()?.into()).await?;
                        if let Some(user) = user {
                            participants.push(user)
                        }
                    }

                    let details = if is_group {
                        let name: String = row.get_by_name("name")?;
                        let avatar_hash: String = row.get_by_name("avatar_hash")?;
                        let username: String = row.get_by_name("username")?;

                        Some(ChatDetails {
                            name,
                            chat_id,
                            is_group: true,
                            photo: if !avatar_hash.is_empty() {
                                Some(avatar_hash)
                            } else {
                                None
                            },
                            username: if !username.is_empty() {
                                Some(username)
                            } else {
                                None
                            },
                        })
                    } else {
                        None
                    };

                    return Ok(Some(Chat::construct(
                        chat_id,
                        is_group,
                        participants,
                        details,
                    )));
                }
                Ok(None)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub async fn create_group(
        &self,
        participants: Vec<UserId>,
        details: ChatDetails,
    ) -> Result<Chat, PPError> {
        let chat_id = rand::thread_rng().gen_range(i32::MIN..-1);
        let insert_query = "INSERT INTO ksp.chats (id, is_group, participants, name, avatar_hash, username) VALUES (?, ?, ?, ?, ?, ?)";

        let mut statement = self.session.statement(insert_query);
        statement.bind_int32(0, chat_id)?;
        statement.bind_bool(1, true)?;

        let mut list = cassandra_cpp::List::new();
        for participant in participants {
            match participant {
                UserId::UserId(user_id) => {
                    list.append_int32(user_id)?;
                }
                UserId::Username(_) => return Err(PPError::from("Cannot add chat with username!")),
            }
        }
        statement.bind_list(2, list)?;
        statement.bind_string(3, details.name())?;
        statement.bind_string(4, details.photo().map_or("", |v| v))?;
        statement.bind_string(5, details.username().map_or("", |v| v))?;

        statement.execute().await?;

        Ok(self.fetch_chat(chat_id).await.unwrap().unwrap())
    }

    pub async fn add_participant(
        &self,
        chat_id: ChatId,
        participant: &UserId,
    ) -> Result<(), PPError> {
        let update_query = "UPDATE ksp.chats SET participants = participants + ? WHERE id = ?;";

        let mut statement = self.session.statement(update_query);
        let mut list = cassandra_cpp::List::new();
        match participant {
            UserId::UserId(user_id) => {
                list.append_int32(*user_id)?;
            }
            UserId::Username(_) => return Err(PPError::from("UserId must be integer!")),
        }
        statement.bind_list(0, list)?;
        statement.bind_int32(1, chat_id)?;

        statement.execute().await?;

        Ok(())
    }

    pub async fn chat_exists(&self, chat_id: ChatId) -> PPResult<bool> {
        let query = "SELECT * FROM ksp.chats WHERE id = ?";

        let mut statement = self.session.statement(query);
        statement.bind_int32(0, chat_id)?;
        let res = statement.execute().await?;

        Ok(res.first_row().is_some())
    }

    /// Fetch chat by real chat id
    pub async fn fetch_chat(&self, chat_id: ChatId) -> Result<Option<Chat>, PPError> {
        let select_query = "SELECT * FROM ksp.chats WHERE id = ?";

        let mut statement = self.session.statement(select_query);
        statement.bind_int32(0, chat_id)?;
        let result = statement.execute().await?;

        if let Some(row) = result.first_row() {
            let chat_id: i32 = row.get_by_name("id")?;
            let is_group: bool = row.get_by_name("is_group")?;

            let mut iter: SetIterator = row.get_by_name("participants")?;
            let users_db: UsersDB = DatabaseBuilder::from_raw(self.session.clone()).into();

            let mut participants: Vec<User> = vec![];
            while let Some(participant) = iter.next() {
                let user = users_db.fetch_user(&participant.get_i32()?.into()).await?;
                if let Some(user) = user {
                    participants.push(user)
                }
            }

            let details = if is_group {
                let name: String = row.get_by_name("name")?;
                let avatar_hash: String = row.get_by_name("avatar_hash")?;
                let username: String = row.get_by_name("username")?;

                Some(ChatDetails {
                    name,
                    chat_id,
                    is_group: true,
                    photo: if !avatar_hash.is_empty() {
                        Some(avatar_hash)
                    } else {
                        None
                    },
                    username: if !username.is_empty() {
                        Some(username)
                    } else {
                        None
                    },
                })
            } else {
                None
            };

            return Ok(Some(Chat::construct(
                chat_id,
                is_group,
                participants,
                details,
            )));
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
