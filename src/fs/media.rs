use core::error;
use std::borrow::Cow;
use std::cell::OnceCell;
use std::fmt::format;
use std::io::ErrorKind;
use std::path::PathBuf;

use log::{info, error};
use tokio::fs as filesystem;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

use crate::db::internal::error::PPError;

use super::hasher::BinaryHasher;

fn get_target_media_dir(sha256_encoded_hash: &String) -> Result<PathBuf, PPError> {
    let path = std::env::current_dir().map_err(|err| PPError::from(err.to_string()))?;
    Ok(path.join(sha256_encoded_hash))
}

async fn put_media(media_name: &String, sha256_encoded_hash: &String, binary: &Vec<u8>) -> Result<(), PPError> {
    let mut path = get_target_media_dir(sha256_encoded_hash)?;

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

pub async fn media_exists(binary: &Vec<u8>) -> Result<bool, PPError> {
    let mut hasher = BinaryHasher::new();
    hasher.hash_part(&binary);
    let sha256_encoded_hash = hasher.finalize();

    let exists = filesystem::try_exists(get_target_media_dir(&sha256_encoded_hash)?).await.map_err(|err| {
        error!("{}", err);
        PPError::from(err.to_string())
    })?;

    Ok(exists)
}

pub async fn media_hash_exists(media_hash: &String) -> Result<bool, PPError> {
    let exists = filesystem::try_exists(get_target_media_dir(&media_hash)?).await.map_err(|err| {
        error!("{}", err);
        PPError::from(err.to_string())
    })?;

    Ok(exists)
}

pub async fn get_media(media_hash: &String) -> Result<Vec<u8>, PPError> {
    let path = get_target_media_dir(media_hash)?;

    if !media_hash_exists(media_hash).await? {
        return Err(PPError::from("Media with the given hash doesn't exist"));
    }

    let mut file = filesystem::File::open(path.join("file")).await.map_err(|err| {
        error!("{}", err);
        PPError::from("Internal Filesystem error.")
    })?;

    let mut file_data: Vec<u8> = vec![];
    file.read_to_end(&mut file_data).await.map_err(|err| {
        error!("{}", err);
        PPError::from("Internal Filesystem error.")
    })?;

    Ok(file_data)
}

// Pass in encoded base64 binary.
pub async fn add_media(binary: &Vec<u8>) -> Result<String, PPError> {
    let mut hasher = BinaryHasher::new();
    hasher.hash_part(&binary);
    let sha256_encoded_hash = hasher.finalize();

    put_media(&"file".into(), &sha256_encoded_hash, binary).await?;

    Ok(sha256_encoded_hash)
}