use core::error;
use std::borrow::Cow;
use std::fmt::format;
use std::io::ErrorKind;
use std::path::PathBuf;

use log::{info, error};
use tokio::fs as filesystem;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

use crate::db::internal::error::PPError;

use super::hasher::BinaryHasher;

const BASE_DIRECTORY_PREFIX: &str = "~/server_media/";

async fn put_media(media_name: &String, sha256_encoded_hash: &String, binary: &Vec<u8>) -> Result<(), PPError> {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| String::from("/"));
    let base_directory: PathBuf = BASE_DIRECTORY_PREFIX.replace("~", &home_dir).into();

    let mut path = base_directory;
    path = path.join(sha256_encoded_hash);

    info!("{}", path.display());

    match filesystem::create_dir(&path).await {
        Ok(_) => {
            path = path.join(media_name);
            let maybe_file = filesystem::File::create(path).await;

            match maybe_file {
                Ok(mut file) => {
                    file.write_all(&binary).await.unwrap();
                }
                Err(err) => {
                    info!("{}", err);
                    return Err(PPError::from("This file already exists!"));
                }
            }
        }
        Err(err) => {
            if err.kind() == ErrorKind::AlreadyExists {
                info!("This media: {} already exists!", path.display());
                return Err(PPError::from("This media already exists!"));
            }
            error!("{}", err);
            return Err(PPError::from("Internal Filesystem error."))
        }
    }

    Ok(())
}

// Pass in encoded base64 binary.
pub async fn add_media(binary: &Vec<u8>) -> Result<String, PPError> {
    let mut hasher = BinaryHasher::new();
    hasher.hash_part(&binary);
    let sha256_encoded_hash = hasher.finalize();

    put_media(&"file".into(), &sha256_encoded_hash, binary).await?;

    Ok(sha256_encoded_hash)
}