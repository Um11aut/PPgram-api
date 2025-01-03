use std::{borrow::Cow, path::PathBuf};

use log::{debug, info, warn};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::{
    db::internal::error::{PPError, PPResult},
    fs::document::fetch_metadata,
    server::{message::types::files::Metadata, server::FILES_MESSAGE_ALLOCATION_SIZE},
};

use super::{hash_exists, hasher::BinaryHasher, helpers::compress, FsFetcher, FsUploader, FS_BASE};

pub enum VideoType {
    Mp4,
    Mov,
    WebM,
    FLV,
}

pub enum PhotoType {
    Jpeg,
    JPG,
    PNG,
    Heic,
}

pub enum MediaType {
    Video(VideoType),
    Photo(PhotoType),
}

impl TryFrom<&str> for MediaType {
    type Error = PPError;

    fn try_from(file_name: &str) -> Result<Self, Self::Error> {
        let file_name = file_name.to_lowercase();
        let fmt = file_name
            .rsplit('.')
            .next()
            .ok_or(PPError::from("name must contain the file type!"))?;

        match fmt {
            "mp4" => Ok(Self::Video(VideoType::Mp4)),
            "mov" => Ok(Self::Video(VideoType::Mov)),
            "webm" => Ok(Self::Video(VideoType::WebM)),
            "flv" => Ok(Self::Video(VideoType::FLV)),
            "jpeg" => Ok(Self::Photo(PhotoType::Jpeg)),
            "jpg" => Ok(Self::Photo(PhotoType::JPG)),
            "png" => Ok(Self::Photo(PhotoType::PNG)),
            "heic" => Ok(Self::Photo(PhotoType::Heic)),
            _ => Err(PPError::from("Media type not supported!")),
        }
    }
}

/// Struct for framed uploading of media
///
/// Uploads a binary frame to a random temp_file in TEMPDIR, while generating a SHA256 hash
///
/// Then taking that hash and putting it into according Folder that is named after the hash
pub struct MediaUploader {
    hasher: BinaryHasher,
    temp_file: File,
    temp_file_path: PathBuf,
    doc_name: String,
}

impl MediaUploader {
    pub async fn new(document_name: impl Into<Cow<'static, str>>) -> PPResult<MediaUploader> {
        let media_name = document_name.into().to_string();

        // Depending on the media type make compression
        let _ = MediaType::try_from(media_name.as_str())?;

        // Generating a random temp file where all the framed binary will be put
        let temp_file: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();
        let temp_path = std::env::temp_dir().join(temp_file).canonicalize()?;
        info!(
            "Creating new temp file for media uploading: {}",
            temp_path.display()
        );

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&temp_path)
            .await?;

        Ok(MediaUploader {
            hasher: BinaryHasher::new(),
            temp_file: file,
            temp_file_path: temp_path,
            doc_name: media_name,
        })
    }
}

#[async_trait::async_trait]
impl FsUploader for MediaUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(part).await?;
        self.hasher.hash_part(part);

        Ok(())
    }

    async fn finalize(self) -> String {
        let buf = PathBuf::from(FS_BASE);
        if !buf.exists() {
            tokio::fs::create_dir(buf.canonicalize().unwrap())
                .await
                .unwrap();
        }

        // Getting full sha256 hash
        let sha256_hash = self.hasher.finalize();
        let target_doc_directory = buf.join(&sha256_hash);

        // If document already exists, delete the temporary file.
        if target_doc_directory.exists() {
            warn!(
                "The media hash {} already exists... Deleting temporary file. Path: {}",
                sha256_hash,
                self.temp_file_path.display()
            );
            tokio::fs::remove_file(self.temp_file_path).await.unwrap();
            return sha256_hash;
        }

        tokio::fs::create_dir(&target_doc_directory).await.unwrap();
        tokio::fs::rename(
            self.temp_file_path,
            target_doc_directory.join(&self.doc_name),
        )
        .await
        .unwrap();

        let dot_pos = self.doc_name.rfind('.').unwrap();
        let (name, extension) = self.doc_name.split_at(dot_pos);
        let preview_name = format!("{}preview.{}", name, extension);

        if let Err(err) = compress::generate_thumbnail(
            target_doc_directory.join(self.doc_name),
            target_doc_directory.join(preview_name),
            compress::ThumbnailQuality::Medium
        )
        {
            log::error!("Error occured while generating thumbnail: {}", err);
        }

        sha256_hash
    }
}

pub struct MediaFetcher {
    sha256_hash: String,
    metadatas: Vec<Metadata>,
    bytes_read: u64,
    current_file: Option<File>,
    // To avoid allocating each time
    read_buf: Box<[u8]>,
}

unsafe impl Send for MediaFetcher {}
unsafe impl Sync for MediaFetcher {}

impl MediaFetcher {
    pub fn new(sha256_hash: &str) -> Self {
        // Move buffer to the heap
        let buf = Box::new([0; FILES_MESSAGE_ALLOCATION_SIZE]);

        Self {
            sha256_hash: sha256_hash.to_string(),
            metadatas: vec![],
            bytes_read: 0,
            current_file: None,
            read_buf: buf,
        }
    }
}

#[async_trait::async_trait]
impl FsFetcher for MediaFetcher {
    async fn fetch_metadata(&mut self) -> PPResult<Vec<Metadata>> {
        self.metadatas = fetch_metadata(&self.sha256_hash).await?;

        debug!("Opening first file: {}", self.metadatas[0].file_path);
        self.current_file = Some(File::open(&self.metadatas[0].file_path).await?);

        Ok(self.metadatas.clone())
    }

    /// Allocates some constant Value on the heap
    async fn fetch_part(&mut self) -> PPResult<Vec<u8>> {
        if self.metadatas.is_empty() {
            warn!("Cannot fetch anything more...");
            return Ok(vec![]);
        }

        if let Some(current_file) = self.current_file.as_mut() {
            let read = current_file.read(&mut self.read_buf[..]).await?;

            self.bytes_read += read as u64;

            // Then file is finished reading
            // Open the next one
            if read == 0 {
                self.metadatas.drain(..1);

                if self.metadatas.is_empty() {
                    return Ok(self.read_buf[..read].to_vec());
                }

                if let Some(metadata) = self.metadatas.iter().next() {
                    info!("Opening new file: {}!", metadata.file_path);
                    self.current_file = Some(File::open(&metadata.file_path).await?);
                    self.bytes_read = 0;
                }
            }

            return Ok(self.read_buf[..read].to_vec());
        } else {
            return Err(PPError::from("File isn't opened!"));
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

/// If has more than 2 files - is media
pub async fn is_media(sha256_hash: &str) -> PPResult<bool> {
    if !hash_exists(sha256_hash).await? {
        return Err("Given SHA256 Hash doesn't exist!".into());
    };
    let mut entries = tokio::fs::read_dir(PathBuf::from(FS_BASE).join(sha256_hash)).await?;

    let mut count = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_type().await?.is_file() {
            // Will panic on windows when the path is not in ASCII
            let file_name = entry.file_name().into_string().unwrap();
            let file_format = file_name
                .rsplit('.')
                .next()
                .expect("Entry must have a file_name!");

            let maybe_media = MediaType::try_from(file_format);
            if maybe_media.is_err() {
                return Ok(false);
            }

            count += 1;
        }

        if count > 1 {
            return Ok(true);
        }
    }

    Ok(false)
}
