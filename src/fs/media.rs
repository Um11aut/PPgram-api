use std::fmt::format;
use std::io::ErrorKind;
use std::path::PathBuf;

use log::info;
use tokio::fs as filesystem;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

use crate::db::internal::error::PPError;
use crate::db::internal::hasher::BinaryHasher;

const BASE_DIRECTORY_PREFIX: &str = "~/server_media/";

async fn put_media(media_name: String, sha256_encoded_hash: String, binary: Vec<u8>) -> Result<(), PPError> {
    let mut path = PathBuf::from(BASE_DIRECTORY_PREFIX);
    path = path.join(sha256_encoded_hash);
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
        }
    }

    Ok(())
}

// Pass in encoded base64 binary.
pub async fn create_media(encoded_binary: String, media_name: String) -> Result<(), PPError> {
    let hasher = BinaryHasher::new();
    let (decoded_binary, sha256_encoded_hash) = hasher
        .hash_full(&encoded_binary)
        .map_err(|err| PPError::from(err.to_string()))?;

    put_media(media_name, sha256_encoded_hash, decoded_binary).await?;

    Ok(())
}