use std::{borrow::Cow, path::PathBuf};

use log::{debug, info, warn};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{fs::{File, OpenOptions}, io::{AsyncReadExt, AsyncWriteExt}};

use crate::{db::internal::error::{PPError, PPResult}, server::{message::types::files::Metadata, server::FILES_MESSAGE_ALLOCATION_SIZE}};

use super::{hasher::BinaryHasher, FsFetcher, FsUploader, FS_BASE};

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
        let temp_path = std::env::temp_dir().join(temp_file);
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
impl FsUploader for DocumentUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(&part).await?;
        self.hasher.hash_part(&part);

        Ok(())
    }

    async fn finalize(self) -> String {
        let buf = PathBuf::from(FS_BASE);
        if !buf.exists() {
            tokio::fs::create_dir(&buf).await.unwrap();
        }

        // Getting full sha256 hash
        let sha256_hash = self.hasher.finalize();
        let target_doc_directory = buf.join(&sha256_hash);

        // If document already exists, delete the temporary file.
        if target_doc_directory.exists() {
            warn!("The media hash {} already exists... Deleting temporary file. Path: {}", sha256_hash, self.temp_file_path.display());
            tokio::fs::remove_file(&self.temp_file_path).await.unwrap();
            return sha256_hash;
        }

        tokio::fs::create_dir(&target_doc_directory).await.unwrap();
        tokio::fs::rename(&self.temp_file_path, target_doc_directory.join(&self.doc_name)).await.unwrap();

        sha256_hash
    }
}

pub struct DocumentFetcher {
    sha256_hash: String,
    metadatas: Vec<Metadata>,
    bytes_read: u64,
    current_file: Option<File>,
}

unsafe impl Send for DocumentFetcher {}
unsafe impl Sync for DocumentFetcher {}

impl DocumentFetcher {
    pub fn new(sha256_hash: &str) -> Self {
        Self {
            sha256_hash: sha256_hash.to_string(),
            metadatas: vec![],
            bytes_read: 0,
            current_file: None,
        }
    }
}

#[async_trait::async_trait]
impl FsFetcher for DocumentFetcher {
    async fn fetch_metadata(&mut self) -> PPResult<Vec<Metadata>> {
        let mut entries = tokio::fs::read_dir(PathBuf::from(FS_BASE).join(&self.sha256_hash)).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path().canonicalize()?;
            let name = entry.file_name().into_string().unwrap();
            let metadata = entry.metadata().await?;

            self.metadatas.push(Metadata{
                file_name: name,
                file_path: path.to_string_lossy().to_string(),
                file_size: metadata.len()
            })
        }

        // Sort from smallest to biggest file size
        self.metadatas.sort_by(|a,b| a.file_size.cmp(&b.file_size));

        debug!("Opening first file: {}", self.metadatas[0].file_path);
        self.current_file = Some(File::open(&self.metadatas[0].file_path).await?);

        Ok(self.metadatas.clone())
    }

    /// Allocates some constant Value on the heap
    async fn fetch_part(&mut self) -> PPResult<Vec<u8>> {
        if self.metadatas.is_empty() {
            warn!("Cannot fetch anything more...");
            return Ok(vec![])
        }

        // Move buffer to the heap
        let mut buf = Box::new([0; FILES_MESSAGE_ALLOCATION_SIZE]);

        if let Some(current_file) = self.current_file.as_mut() {
            let read = current_file.read(&mut buf[..]).await?;

            self.bytes_read += read as u64;

            // Then file is finished reading
            // Open the next one
            if read == 0 {
                self.metadatas.drain(..1);

                if self.metadatas.is_empty() {return Ok(buf[..read].to_vec())}

                if let Some(metadata) = self.metadatas.iter().next() {
                    info!("Opening new file: {}!", metadata.file_path);
                    self.current_file = Some(File::open(&metadata.file_path).await?);
                    self.bytes_read = 0;
                }
            }

            return Ok(buf[..read].to_vec())
        } else {
            return Err(PPError::from("File isn't opened!"))
        }
    }

    fn is_part_ready(&self) -> bool {
        if let Some(current) = self.metadatas.first() {
            return self.bytes_read == current.file_size;
        } else {
            return true;
        }
    }
}
