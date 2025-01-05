use log::debug;
use tokio::{fs::File, io::AsyncReadExt};

use crate::{
    db::{
        chat::hashes::HashesDB,
        internal::error::{PPError, PPResult},
    },
    fs::document::fetch_hash_metadata,
    server::{message::types::files::Metadata, server::FILES_MESSAGE_ALLOCATION_SIZE},
};

pub enum MediaFetchMode {
    PreviewOnly,
    MediaOnly,
    Full,
}

impl TryFrom<&str> for MediaFetchMode {
    type Error = PPError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "preview_only" => Ok(Self::PreviewOnly),
            "media_only" => Ok(Self::MediaOnly),
            "full" => Ok(Self::Full),
            _ => Err("Unkown mode provided. Known modes: preview_only, media_only, full".into()),
        }
    }
}

pub(crate) struct FileFetcher {
    metadatas: (Option<Metadata>, Option<Metadata>),
    current_file: File,
    read_buf: Box<[u8]>,
}

impl FileFetcher {
    pub async fn new(db: HashesDB, sha256_hash: String, mode: MediaFetchMode) -> PPResult<Self> {
        let buf = Box::new([0; FILES_MESSAGE_ALLOCATION_SIZE]);

        let hash_info = db
            .fetch_hash(&sha256_hash)
            .await?
            .ok_or("Provided SHA256 Hash doesn't exist")?;

        let (main_metadata, maybe_preview) = fetch_hash_metadata(db, &sha256_hash).await?;

        // the metadatas are sorted in size ascending order and can have only 1 preview per hash
        let metadatas = if hash_info.is_media {
            #[cfg(debug_assertions)]
            assert!(maybe_preview.is_some());

            match mode {
                MediaFetchMode::PreviewOnly => (None, maybe_preview),
                MediaFetchMode::MediaOnly => (Some(main_metadata), None),
                MediaFetchMode::Full => (Some(main_metadata), maybe_preview),
            }
        } else {
            (Some(main_metadata), None)
        };

        let current_file = if let Some(preview_mt) = metadatas.1.as_ref() {
            File::open(&preview_mt.file_path).await?
        } else if let Some(main_mt) = metadatas.0.as_ref() {
            File::open(&main_mt.file_path).await?
        } else {
            unreachable!()
        };

        Ok(Self {
            metadatas,
            current_file,
            read_buf: buf,
        })
    }

    pub fn get_metadata(&self) -> (Option<Metadata>, Option<Metadata>) {
        self.metadatas.clone()
    }

    /// Fetch bytes part
    pub async fn fetch_data_frame(&mut self) -> PPResult<&[u8]> {
        if self.metadatas.1.is_some() {
            let bytes_read = self.current_file.read(&mut self.read_buf[..]).await?;

            if bytes_read == 0 {
                self.metadatas.1.take();
                if let Some(main_mt) = self.metadatas.0.as_ref() {
                    self.current_file = File::open(&main_mt.file_path).await?;
                }
            }

            Ok(&self.read_buf[..bytes_read])
        } else if self.metadatas.0.is_some() {
            let bytes_read = self.current_file.read(&mut self.read_buf[..]).await?;

            if bytes_read == 0 {
                self.metadatas.0.take();
            }

            Ok(&self.read_buf[..bytes_read])
        } else {
            Ok(&self.read_buf[..0])
        }
    }

    pub fn is_finished(&self) -> bool {
        self.metadatas.0.is_none() && self.metadatas.1.is_none()
    }
}
