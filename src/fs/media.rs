use std::{borrow::Cow, path::PathBuf};

use log::{debug, info, warn};
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
    fs::document::fetch_hash_metadata,
    server::{message::types::files::Metadata, server::FILES_MESSAGE_ALLOCATION_SIZE},
};

use super::{hasher::BinaryHasher, helpers::compress, FsUploader, FS_BASE};

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

impl FsUploader for MediaUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(part).await?;
        self.hasher.hash_part(part);

        Ok(())
    }

    async fn finalize(self, db: &HashesDB) -> PPResult<String> {
        let buf = PathBuf::from(FS_BASE);
        if !buf.exists() {
            tokio::fs::create_dir(buf.canonicalize().unwrap()).await?;
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
            tokio::fs::remove_file(self.temp_file_path).await?;
            return Ok(sha256_hash);
        }

        tokio::fs::create_dir(&target_doc_directory).await?;
        tokio::fs::rename(
            self.temp_file_path,
            target_doc_directory.join(&self.doc_name),
        )
        .await?;

        let dot_pos = self.doc_name.rfind('.').unwrap();
        let (name, extension) = self.doc_name.split_at(dot_pos);
        let preview_name = format!("{}preview.{}", name, extension);

        let file_path = target_doc_directory.join(self.doc_name);
        let preview_path = target_doc_directory.join(preview_name);

        compress::generate_thumbnail(&file_path, &preview_path, compress::ThumbnailQuality::Bad)?;

        db.add_hash(
            true,
            &sha256_hash,
            file_path
                .to_str()
                .ok_or("Failed to convert file path to string.")?,
            Some(
                preview_path
                    .to_str()
                    .ok_or("Failed to convert preview path to string.")?,
            ),
        )
        .await?;

        Ok(sha256_hash)
    }
}
