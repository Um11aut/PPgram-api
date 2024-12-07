use crate::{db::internal::error::PPResult, fs::{document::DocumentFetcher, media::{is_media, MediaFetcher}, FsFetcher}, server::message::types::files::Metadata};

enum Fetcher {
    Document(DocumentFetcher),
    Media(MediaFetcher),
}

impl Fetcher {
    pub async fn fetch_metadata(&mut self) -> PPResult<Vec<Metadata>> {
        match self {
            Fetcher::Document(fetcher) => fetcher.fetch_metadata().await,
            Fetcher::Media(fetcher) => fetcher.fetch_metadata().await,
        }
    }

    pub async fn fetch_part(&mut self) -> PPResult<Vec<u8>> {
        match self {
            Fetcher::Document(fetcher) => fetcher.fetch_part().await,
            Fetcher::Media(fetcher) => fetcher.fetch_part().await,
        }
    }

    pub fn is_part_ready(&self) -> bool {
        match self {
            Fetcher::Document(fetcher) => fetcher.is_part_ready(),
            Fetcher::Media(fetcher) => fetcher.is_part_ready(),
        }
    }
}

pub(crate) struct FileFetcher {
    fetcher: Fetcher,
    /// Metadata of the file will be fetched in runtime
    metadata: Vec<Metadata>,
}

impl FileFetcher {
    pub async fn new(sha256_hash: String, previews_only: bool) -> PPResult<Self> {
        let is_media = is_media(sha256_hash.as_str()).await?;
        let mut fetcher = if is_media {
            Fetcher::Media(MediaFetcher::new(&sha256_hash))
        } else {
            Fetcher::Document(DocumentFetcher::new(&sha256_hash))
        };
        let mut metadata = fetcher.fetch_metadata().await?;

        // the metadatas are sorted in size ascending order and can have only 1 preview per hash
        if previews_only {
            if is_media {
                metadata.drain(1..);
            } else {
                return Err("Loading the preview of documents is not possible!".into());
            }
        }

        Ok(Self { fetcher, metadata })
    }

    pub async fn fetch_metadata_only(sha256_hash: String) -> PPResult<Vec<Metadata>> {
        let is_media = is_media(sha256_hash.as_str()).await?;
        let mut fetcher = if is_media {
            Fetcher::Media(MediaFetcher::new(&sha256_hash))
        } else {
            Fetcher::Document(DocumentFetcher::new(&sha256_hash))
        };
        let metadata = fetcher.fetch_metadata().await?;
        Ok(metadata)
    }

    /// Fetch bytes part
    pub async fn fetch_data_frame(&mut self) -> PPResult<Vec<u8>> {
        if self.fetcher.is_part_ready() {
            self.metadata.drain(..1);
        }
        self.fetcher.fetch_part().await
    }

    /// Gets metadata
    ///
    /// Dangerous: Metadata get's dynamic unloaded while calling `get_built_response`
    pub fn get_metadata(&self) -> Vec<Metadata> {
        self.metadata.clone()
    }

    pub fn is_finished(&self) -> bool {
        self.metadata.is_empty()
    }
}
