use std::{
    borrow::Borrow,
    net::{Ipv4Addr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};

use quinn::{
    crypto::rustls::QuicServerConfig,
    rustls::{self, pki_types::PrivatePkcs8KeyDer},
};

use crate::{
    db::internal::error::PPResult,
    fs::{document::DocumentUploader, media::MediaUploader, FileUploader},
    server::message::{
        builder::MessageBuilder,
        types::metadata::{self, FilesMetadataMessage},
        Handler,
    },
};

const MAX_MSG_SIZE: u64 = 4 * (1024 * 1024 * 1024 * 1024) /* Gib */; // Max message size that can be transmitted

/// Quic Handler to handle QUIC messages, meaning documents or media.
///
/// The Metadata of the messages goes in JSON:
/// ```
/// [4 bytes metadata message size] [{"name": "Test123", "compress": true}] [8 bytes file message size] [message bytes]
/// ```
pub struct QuicHandler {
    // to put the file: Media or Document will be decided at runtime
    fs_handler: Option<Box<dyn FileUploader + Send>>,
    is_first: bool,
    // MessageBuilder for metadata
    metadata_builder: Option<MessageBuilder>,
    // For file itself
    file_size: u64,
    file_bytes_uploaded: u64,
}

#[async_trait::async_trait]
impl Handler for QuicHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        // For start message, it's not 0
        // needed to determine when file actually starts
        let mut content_start_offset: usize = 0;

        if self.is_first {
            self.metadata_builder = MessageBuilder::parse(&buffer);
            self.is_first = false;
        }

        // Then metadata isn't loaded yet
        if self.file_size == 0 {
            if let Some(metadata_builder) = self.metadata_builder.as_mut() {
                if metadata_builder.ready() {
                    let metadata = metadata_builder.content_utf8().map(|v| {
                        serde_json::from_str::<FilesMetadataMessage>(v)
                            .ok()
                            .unwrap()
                    });

                    // The offset of metadata message to separate metadata and content
                    let metadata_offset: usize = (metadata_builder.size() + 4).try_into().unwrap(); // 4 bytes for metadata message size
                    let content_start = &buffer[metadata_offset..];

                    content_start_offset = metadata_offset + 8; // 8 bytes for files message size

                    self.file_size = u64::from_be_bytes([
                        content_start[0],
                        content_start[1],
                        content_start[2],
                        content_start[3],
                        content_start[4],
                        content_start[5],
                        content_start[6],
                        content_start[7],
                    ]);

                    if let Some(metadata) = metadata {
                        match metadata.is_media {
                            true => {
                                self.fs_handler =
                                    Some(Box::new(MediaUploader::new(metadata.name).await.unwrap()))
                            }
                            false => {
                                self.fs_handler = Some(Box::new(
                                    DocumentUploader::new(metadata.name).await.unwrap(),
                                ))
                            }
                        }
                    }

                    metadata_builder.clear();
                    self.metadata_builder = None;
                }
            }
        }

        // file binary fragment
        let content_fragment = &buffer[content_start_offset..];

        if self.file_size != self.file_bytes_uploaded {
            if let Some(fs_handler) = self.fs_handler.as_mut() {
                let res = fs_handler.upload_part(content_fragment).await;
                self.file_bytes_uploaded += content_fragment.len() as u64;
            }
        }

        // If it's final segment, then finalize the FileUploader
        if self.file_size == self.file_bytes_uploaded {
            if let Some(fs_handler) = self.fs_handler.take() {
                fs_handler.finalize().await;
            }
        }
    }
}

impl QuicHandler {
    pub async fn new() -> QuicHandler {

        QuicHandler {
            fs_handler: None,
            is_first: true,
            metadata_builder: None,
            file_size: 0,
            file_bytes_uploaded: 0,
        }
    }
}
