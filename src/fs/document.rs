use std::{borrow::Cow, path::PathBuf};

use log::{debug, error, info, warn};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::{
    db::{
        chat::hashes::HashesDB,
        internal::error::{PPError, PPResult},
    },
    server::{message::types::files::Metadata, server::FILES_MESSAGE_ALLOCATION_SIZE},
};

use super::{hasher::BinaryHasher, FsUploader, FS_BASE};

/// Struct for framed uploading of documents
///
/// Uploads a binary frame to a random temp_file in TEMPDIR, while generating a SHA256 hash
///
/// Then taking that hash and putting it into according Folder that is named after the hash
pub struct DocumentUploader {
    hasher: BinaryHasher,
    temp_file: File,
    temp_file_path: PathBuf,
    doc_name: String,
}

impl DocumentUploader {
    pub async fn new(document_name: impl Into<Cow<'static, str>>) -> PPResult<DocumentUploader> {
        // Generating a random temp file where all the framed binary will be put
        let temp_file: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();
        let temp_path = std::env::temp_dir().join(temp_file);
        info!(
            "Creating new temp file for document uploading: {}",
            temp_path.display()
        );

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&temp_path)
            .await?;

        Ok(DocumentUploader {
            hasher: BinaryHasher::new(),
            temp_file: file,
            temp_file_path: temp_path,
            doc_name: document_name.into().to_string(),
        })
    }
}

impl FsUploader for DocumentUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(part).await?;
        self.hasher.hash_part(part);

        Ok(())
    }

    async fn finalize(self, db: &HashesDB) -> PPResult<String> {
        let buf = PathBuf::from(FS_BASE);
        if !buf.exists() {
            tokio::fs::create_dir(&buf).await?;
        }

        // Getting full sha256 hash
        let sha256_hash = self.hasher.finalize();
        let target_doc_directory = buf.join(&sha256_hash);

        // If document already exists, delete the temporary file.
        if target_doc_directory.exists() {
            warn!(
                "The document hash {} already exists... Deleting temporary file. Path: {}",
                sha256_hash,
                self.temp_file_path.display()
            );
            tokio::fs::remove_file(&self.temp_file_path).await?;
            return Ok(sha256_hash);
        }

        let file_path = target_doc_directory.join(&self.doc_name);

        tokio::fs::create_dir(&target_doc_directory).await?;
        tokio::fs::rename(&self.temp_file_path, &file_path).await?;

        db.add_hash(
            false,
            &sha256_hash,
            file_path
                .to_str()
                .ok_or("Failed to convert file path to string.")?,
            None,
        )
        .await?;

        Ok(sha256_hash)
    }
}

pub async fn fetch_hash_metadata(
    db: HashesDB,
    sha256_hash: &str,
) -> PPResult<(Metadata, Option<Metadata>)> {
    let hash_info = db
        .fetch_hash(&sha256_hash)
        .await?
        .ok_or("Provided SHA256 Hash doesn't exist")?;

    if !tokio::fs::try_exists(&hash_info.file_path).await? {
        error!("Provided in database preview path doesn't exist on FS!");
        return Err("Internal error.".into());
    }

    if let Some(preview_path) = hash_info.preview_path.as_ref() {
        if !tokio::fs::try_exists(preview_path).await? {
            error!("Provided in database preview path doesn't exist on FS!");
            return Err("Internal error.".into());
        }
    }

    let main_metadata = tokio::fs::metadata(&hash_info.file_path).await?;

    let preview_metadata = if let Some(preview_path) = hash_info.preview_path.as_ref() {
        Some(Metadata {
            file_name: preview_path
                .to_string_lossy()
                .rsplit('.')
                .next()
                .expect("file name to exist")
                .into(),
            file_path: preview_path.to_string_lossy().into(),
            file_size: tokio::fs::metadata(preview_path).await?.len(),
        })
    } else {
        None
    };

    Ok((
        Metadata {
            file_name: hash_info
                .file_path
                .to_string_lossy()
                .rsplit('.')
                .next()
                .expect("file name to exist")
                .into(),
            file_path: hash_info.file_path.to_string_lossy().into(),
            file_size: main_metadata.len(),
        },
        preview_metadata,
    ))
}
