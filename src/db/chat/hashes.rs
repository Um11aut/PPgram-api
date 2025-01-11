use std::{path::PathBuf, sync::Arc};

use futures::TryStreamExt;

use crate::db::{
    bucket::DatabaseBuilder,
    init::Database,
    internal::error::{PPError, PPResult},
};

#[derive(Clone)]
pub struct HashesDB {
    session: Arc<scylla::Session>,
}

impl From<DatabaseBuilder> for HashesDB {
    fn from(value: DatabaseBuilder) -> Self {
        HashesDB {
            session: value.bucket.get_connection(),
        }
    }
}

impl Database for HashesDB {
    fn new(session: Arc<scylla::Session>) -> Self {
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

        self.session.query_unpaged(create_table_query, &[]).await?;
        Ok(())
    }
}

pub struct HashInfo {
    pub is_media: bool,
    pub file_name: String,
    pub file_path: PathBuf,
    pub preview_path: Option<PathBuf>,
}

impl HashesDB {
    pub async fn hash_exists(&self, sha256_hash: &str) -> PPResult<bool> {
        let query = r#"
            SELECT hash
            FROM ksp.hashes
            WHERE hash = ?;
        "#;

        let result = self
            .session
            .query_unpaged(query, (sha256_hash.to_owned(),))
            .await?;

        Ok(result.is_rows())
    }

    pub async fn fetch_hash(&self, sha256_hash: &str) -> PPResult<Option<HashInfo>> {
        let query = r#"
            SELECT is_media, file_name, file_path, preview_path
            FROM ksp.hashes
            WHERE hash = ?;
        "#;

        let prepared = self.session.prepare(query).await?;
        let result = self
            .session
            .execute_iter(prepared, (sha256_hash,))
            .await?
            .rows_stream::<(bool, String, String, String)>()?
            .try_next()
            .await?;

        if let Some((is_media, file_name, file_path, preview_path)) = result {
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
            VALUES (?, ?, ?, ?, ?);
        "#;

        let prepared = self.session.prepare(query).await?;
        self.session
            .execute_unpaged(
                &prepared,
                (
                    sha256_hash,
                    is_media,
                    file_name.to_owned(),
                    file_path.to_owned(),
                    preview_path.unwrap_or(""),
                ),
            )
            .await?;

        Ok(())
    }
}
