use std::{borrow::Cow, path::{Path, PathBuf}};

use log::{info, warn};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{fs::{File, OpenOptions}, io::AsyncWriteExt};

use crate::db::internal::error::PPResult;

use super::{hasher::BinaryHasher, FileUploader, FS_BASE};

/// Struct for framed uploading of documents
/// 
/// Uploads a binary frame to a random temp_file in TEMPDIR, while generating a SHA256 hash
/// 
/// Then taking that hash and putting it into according Folder that is named after the hash
pub struct DocumentUploader {
    hasher: BinaryHasher,
    temp_file: File,
    temp_file_path: PathBuf,
    doc_name: String
}

impl DocumentUploader {
    pub async fn new(document_name: impl Into<Cow<'static, str>>) -> PPResult<DocumentUploader> {
        // Generating a random temp file where all the framed binary will be put
        let temp_file: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();
        let temp_path = std::env::temp_dir().join(temp_file).canonicalize()?;
        info!("Creating new temp file for document uploading: {}", temp_path.display());

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
            doc_name: document_name.into().to_string()
        })
    }
}

#[async_trait::async_trait]
impl FileUploader for DocumentUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(&part).await?;
        self.hasher.hash_part(&part);

        Ok(())
    }

    async fn finalize(self: Box<Self>) {
        let buf = PathBuf::from(FS_BASE);
        if !buf.exists() {
            tokio::fs::create_dir(buf.canonicalize().unwrap()).await.unwrap();
        }

        // Getting full sha256 hash
        let sha256_hash = self.hasher.finalize();
        let target_doc_directory = buf.join(&sha256_hash);

        // If document already exists, delete the temporary file.
        if target_doc_directory.exists() {
            warn!("The media hash {} already exists... Deleting temporary file. Path: {}", sha256_hash, self.temp_file_path.display());
            tokio::fs::remove_file(&self.temp_file_path).await.unwrap();
            return;
        }

        tokio::fs::create_dir(&target_doc_directory).await.unwrap();
        tokio::fs::rename(&self.temp_file_path, target_doc_directory.join(&self.doc_name)).await.unwrap();
    }
}