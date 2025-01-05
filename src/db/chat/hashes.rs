use std::{path::PathBuf, sync::Arc};

use cassandra_cpp::AsRustType;

use crate::db::{
    bucket::DatabaseBuilder,
    db::Database,
    internal::error::{PPError, PPResult},
};

#[derive(Clone)]
pub struct HashesDB {
    session: Arc<cassandra_cpp::Session>,
}

impl From<DatabaseBuilder> for HashesDB {
    fn from(value: DatabaseBuilder) -> Self {
        HashesDB {
            session: value.bucket.get_connection(),
        }
    }
}

impl Database for HashesDB {
    fn new(session: Arc<cassandra_cpp::Session>) -> Self {
        Self {
            session: Arc::clone(&session),
        }
    }

    async fn create_table(&self) -> Result<(), PPError> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS ksp.hashes (
                hash TEXT,
                is_media boolean,
                file_name TEXT,
                file_path TEXT,
                preview_path TEXT,
                PRIMARY KEY (hash)
            );
        "#;

        self.session.execute(create_table_query).await?;
        Ok(())
    }
}

pub struct HashInfo {
    pub is_media: bool,
    pub file_name: String,
    pub file_path: PathBuf,
    pub preview_path: Option<PathBuf>
}

impl HashesDB {
    pub async fn hash_exists(&self, sha256_hash: &str) -> PPResult<bool> {
        let query = r#"
            SELECT hash
            FROM ksp.hashes
            WHERE hash = ?;
        "#;

        let mut statement = self.session.statement(query);
        statement.bind_string(0, sha256_hash)?; // Bind the hash

        let result = statement.execute().await?;

        Ok(result.first_row().is_some())
    }

    pub async fn fetch_hash(&self, sha256_hash: &str) -> PPResult<Option<HashInfo>> {
        let query = r#"
            SELECT is_media, file_name, file_path, preview_path
            FROM ksp.hashes
            WHERE hash = ?;
        "#;

        let mut statement = self.session.statement(query);

        statement.bind_string(0, sha256_hash)?;

        let result = statement.execute().await?;

        if let Some(row) = result.first_row() {
            let is_media: bool = row.get(0)?;
            let file_name: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let preview_path: String = row.get(3)?;

            return Ok(Some(HashInfo {
                is_media,
                file_name,
                file_path: file_path.into(),
                preview_path: if preview_path.is_empty() {
                    None
                } else {
                    Some(preview_path.into())
                },
            }));
        }

        Ok(None)
    }

    pub async fn add_hash(
        &self,
        is_media: bool,
        sha256_hash: &str,
        file_name: &str,
        file_path: &str,
        preview_path: Option<&str>,
    ) -> PPResult<()> {
        let query = r#"
            INSERT INTO ksp.hashes (hash, is_media, file_name, file_path, preview_path)
            VALUES (?, ?, ?, ?);
        "#;

        let mut statement = self.session.statement(query);

        statement.bind_string(0, sha256_hash)?; // Bind the hash
        statement.bind_bool(1, is_media)?;
        statement.bind_string(2, file_path)?;
        statement.bind_string(3, file_name)?;
        statement.bind_string(4, preview_path.unwrap_or(""))?;

        statement.execute().await?;

        Ok(())
    }
}
