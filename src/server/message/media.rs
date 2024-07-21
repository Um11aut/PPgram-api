use serde::{Deserialize, Serialize};


// Define a struct to represent a single media item
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaItem {
    pub file_name: String,
    pub format: String,
    pub data: String,
}

// Define a struct to represent media messages with multiple media items
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaMessage {
    pub media: Vec<MediaItem>,
    pub has_caption: bool,
    pub caption: String,
}