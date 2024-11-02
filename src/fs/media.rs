use std::{borrow::Cow, path::{Path, PathBuf}};

use log::{info, warn};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{fs::{File, OpenOptions}, io::AsyncWriteExt};

use crate::db::internal::error::{PPError, PPResult};

use super::{hasher::BinaryHasher, FileUploader, FS_BASE};

pub enum MediaVideoType {
    Mp4,
    Mov,
    WebM,
    FLV
}

pub enum MediaPhotoType {
    Jpeg,
    JPG,
    PNG,
    Heic,
}

pub enum MediaType {
    MediaVideoType(MediaVideoType),
    MediaPhotoType(MediaPhotoType)
}

impl TryFrom<String> for MediaType {
    type Error = PPError;

    fn try_from(file_name: String) -> Result<Self, Self::Error> {
        let file_name = file_name.to_lowercase();
        let fmt = file_name.rsplit('.').next().ok_or(PPError::from("name must contain the file type!"))?;

        match fmt {
            "mp4" => Ok(Self::MediaVideoType(MediaVideoType::Mp4)),
            "mov" => Ok(Self::MediaVideoType(MediaVideoType::Mov)),
            "webm" => Ok(Self::MediaVideoType(MediaVideoType::WebM)),
            "flv" => Ok(Self::MediaVideoType(MediaVideoType::FLV)),
            "jpeg" => Ok(Self::MediaPhotoType(MediaPhotoType::Jpeg)),
            "jpg" => Ok(Self::MediaPhotoType(MediaPhotoType::JPG)),
            "png" => Ok(Self::MediaPhotoType(MediaPhotoType::PNG)),
            "heic" => Ok(Self::MediaPhotoType(MediaPhotoType::Heic)),
            _ => Err(PPError::from("Media type not supported!"))
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
    doc_name: String
}

impl MediaUploader {
    pub async fn new(document_name: impl Into<Cow<'static, str>>) -> PPResult<MediaUploader> {
        let document_name = document_name.into().to_string();

        // TODO: Dependending on the media type create compression support
        let media_type = MediaType::try_from(document_name.clone())?;

        // Generating a random temp file where all the framed binary will be put
        let temp_file: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();
        let temp_path = std::env::temp_dir().join(temp_file).canonicalize()?;
        info!("Creating new temp file for media uploading: {}", temp_path.display());

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
            doc_name: document_name
        })
    }
}

#[async_trait::async_trait]
impl FileUploader for MediaUploader {
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        self.temp_file.write_all(&part).await?;
        self.hasher.hash_part(&part);

        Ok(())
    }

    async fn finalize(self: Box<Self>) -> String {
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
            tokio::fs::remove_file(self.temp_file_path).await.unwrap();
            return sha256_hash;
        }

        tokio::fs::create_dir(&target_doc_directory).await.unwrap();
        tokio::fs::rename(self.temp_file_path, target_doc_directory.join(self.doc_name)).await.unwrap();

        sha256_hash
    }
}