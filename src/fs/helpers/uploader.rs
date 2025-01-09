use crate::{
    db::{chat::hashes::HashesDB, internal::error::{PPError, PPResult}},
    fs::{document::DocumentUploader, media::MediaUploader, FsUploader},
    server::message::types::files::FileMetadataRequest,
};

/// 64Gib - Max message size that can be transmitted
const MAX_MSG_SIZE: u64 = 64 * (1024 * 1024 * 1024) /* Gib */;

enum Uploader {
    Document(DocumentUploader),
    Media(MediaUploader),
}

impl Uploader {
    pub async fn upload_part(&mut self, part: &[u8]) -> PPResult<()> {
        match self {
            Uploader::Document(uploader) => uploader.upload_part(part).await,
            Uploader::Media(uploader) => uploader.upload_part(part).await,
        }
    }

    pub async fn finalize(self, db: &HashesDB) -> PPResult<String> {
        match self {
            Uploader::Document(uploader) => uploader.finalize(db).await,
            Uploader::Media(uploader) => uploader.finalize(db).await,
        }
    }
}

pub(crate) struct FileUploader {
    uploader: Uploader,
    file_size: u64,
    bytes_uploaded: u64,
}

impl FileUploader {
    pub async fn new(metadata: FileMetadataRequest, file_size: u64) -> PPResult<Self> {
        if file_size > MAX_MSG_SIZE {
            return Err(PPError::from("Ты че еблан? Говорили тебе максимум 64гб"));
        }

        let uploader: Uploader = if metadata.is_media {
            Uploader::Media(MediaUploader::new(metadata.name).await?)
        } else {
            Uploader::Document(DocumentUploader::new(metadata.name).await?)
        };

        Ok(Self {
            uploader,
            file_size,
            bytes_uploaded: 0,
        })
    }

    /// Upload file itself
    pub async fn consume_data_frame(&mut self, part: &[u8]) -> PPResult<()> {
        self.uploader.upload_part(part).await?;
        self.bytes_uploaded += part.len() as u64;

        Ok(())
    }

    pub async fn finalize(self, db: &HashesDB) -> PPResult<String> {
        self.uploader.finalize(db).await
    }

    pub fn rest_to_upload(&self) -> u64 {
        self.file_size - self.bytes_uploaded
    }

    pub fn is_ready(&self) -> bool {
        self.file_size == self.bytes_uploaded
    }
}
