use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct FilesMetadataMessage {
    pub name: String,
    pub is_media: bool,
    pub compress: bool
}