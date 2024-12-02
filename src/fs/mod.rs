use std::path::PathBuf;

use crate::{db::internal::error::PPResult, server::message::types::files::Metadata};

pub mod media;
mod hasher;
pub mod document;

/// Base trait for uploading files
#[async_trait::async_trait]
pub trait FsUploader {
    /// Uploads only part of the file to fs
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()>;

    /// Must finalize the upload, giving the binary according full SHA256 hash
    /// and removing it from %TEMP%
    /// Returns SHA256 Hash encoded in hex
    async fn finalize(self: Box<Self>) -> String;
}

#[async_trait::async_trait]
pub trait FsFetcher {
    /// Fetches all files in the directory and returns the metadata(s) of those file
    ///
    /// Length of the Vector is the count of all files stored
    async fn fetch_metadata(&mut self) -> PPResult<Vec<Metadata>>;

    /// Downloads part of the file(s)
    /// sha256 hash must be encoded in hex
    async fn fetch_part(&mut self) -> PPResult<Vec<u8>>;

    /// Indicates if `fetch_part` has fetched the last part of the current file
    fn is_part_ready(&self) -> bool;
}

pub async fn hash_exists(sha256_hash: &str) -> PPResult<bool> {
    let path = PathBuf::from(FS_BASE);
    Ok(tokio::fs::try_exists(path.join(sha256_hash)).await?)
}

const FS_BASE: &str = "/server_data/";
