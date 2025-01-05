use std::path::PathBuf;

use crate::{db::{chat::hashes::HashesDB, internal::error::PPResult}, server::message::types::files::Metadata};

pub mod media;
mod hasher;
pub(super) mod helpers;
pub mod document;

pub trait FsUploader {
    /// Uploads only part of the file to fs
    fn upload_part(&mut self, part: &[u8]) -> impl std::future::Future<Output = PPResult<()>> + Send;

    /// Must finalize the upload, giving the binary according full SHA256 hash
    /// and removing it from %TEMP%
    /// Returns SHA256 Hash encoded in hex
    fn finalize(self, db: &HashesDB)-> impl std::future::Future<Output = PPResult<String>> + Send;
}

const FS_BASE: &str = "/server_data/";
