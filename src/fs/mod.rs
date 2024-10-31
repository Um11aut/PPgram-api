use crate::db::internal::error::PPResult;

pub mod media;
mod hasher;
pub mod document;

/// Base trait for uploading files
#[async_trait::async_trait]
pub trait FileUploader {
    /// Uploads only part of the file to fs
    async fn upload_part(&mut self, part: &[u8]) -> PPResult<()>;

    /// Must finalize the upload, giving the binary according full SHA256 hash
    /// and removing it from %TEMP%
    async fn finalize(self: Box<Self>);
}

const FS_BASE: &str = "/server_data/";