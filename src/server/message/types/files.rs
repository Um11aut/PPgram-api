use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::internal::error::{PPError, PPResult};

#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetadataRequest {
    pub method: String, // upload_file
    pub name: String,
    pub is_media: bool,
    pub compress: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadFileRequest {
    pub method: String, // download_file
    pub sha256_hash: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadFileMetadataResponse {
    pub method: String, // download_file
    pub metadatas: Vec<Metadata>
}

pub fn extract_media_method(content: &String) -> PPResult<String> {
    let val = serde_json::from_str::<Value>(&content)?;

    Ok(val.get("method").ok_or(PPError::from("Failed to get method!"))?.as_str().ok_or("method must be 'str'!")?.to_owned())
}

/// Struct to fetch the info about file
/// 
/// File Path isn't serialized in Json!
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Metadata {
    pub file_name: String,
    #[serde(skip)]
    pub file_path: String,
    pub file_size: u64,
}