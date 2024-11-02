use std::{
    borrow::Borrow,
    net::{Ipv4Addr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};

use log::info;
use tokio::{net::tcp::OwnedReadHalf, sync::Mutex};

use crate::{
    db::internal::error::{PPError, PPResult},
    fs::{document::DocumentUploader, media::MediaUploader, FileUploader},
    server::{
        connection::TCPConnection,
        message::{
            builder::MessageBuilder,
            types::{
                metadata::{self, FilesMetadataMessage},
                response::send::UploadFileResponse,
            },
            Handler,
        },
    },
};

use super::json_handler::SessionArcRwLock;

/// 4Gib - Max message size that can be transmitted
const MAX_MSG_SIZE: u64 = 4 * (1024 * 1024 * 1024 * 1024) /* Gib */;

/// Quic Handler to handle QUIC messages, meaning documents or media.
///
/// The Metadata of the messages goes in JSON:
/// ```
/// [4 bytes metadata message size] [{"name": "Test123", "is_media": false, "compress": true}] [8 bytes file message size] [message bytes]
/// ```
pub struct FilesHandler {
    // to put the file frame on fs: Media or Document will be decided at runtime
    fs_handler: Option<Box<dyn FileUploader + Send>>,
    is_first: bool,
    // MessageBuilder for metadata
    metadata_builder: Option<MessageBuilder>,
    // For file itself
    file_size: u64,
    file_bytes_uploaded: u64,
    output_connection: Arc<TCPConnection>,
    // temp buffer if IP doesn't transfer all the bytes
    temp_buf: Vec<u8>,
}

#[async_trait::async_trait]
impl Handler for FilesHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        // For start message, it's not 0
        // needed to determine when the binary actually starts
        let mut content_start_offset: usize = 0;

        if self.is_first {
            self.metadata_builder = MessageBuilder::parse(&buffer);
            self.is_first = false;
        }

        // Then metadata isn't loaded yet
        if self.file_size == 0 {
            if let Some(metadata_builder) = self.metadata_builder.as_mut() {
                if metadata_builder.ready() {
                    let metadata = metadata_builder
                        .content_utf8()
                        .map(|v| serde_json::from_str::<FilesMetadataMessage>(v).ok());

                    // The offset of metadata message to separate metadata and content
                    let metadata_offset: usize = (metadata_builder.size() + 4).try_into().unwrap(); // 4 bytes for metadata message size
                    let content_start = &buffer[metadata_offset..];

                    // If IP couldn't transfer enough bytes, extend the temp buf
                    self.temp_buf.extend_from_slice(content_start);
                    if self.temp_buf.len() < 8 {
                        return;
                    }

                    content_start_offset = metadata_offset + 8; // 8 bytes for files message size

                    self.file_size = u64::from_be_bytes([
                        self.temp_buf[0],
                        self.temp_buf[1],
                        self.temp_buf[2],
                        self.temp_buf[3],
                        self.temp_buf[4],
                        self.temp_buf[5],
                        self.temp_buf[6],
                        self.temp_buf[7],
                    ]);

                    if self.file_size > MAX_MSG_SIZE {
                        PPError::from(format!(
                            "File too big: {}! Max allowed file size: 4Gib",
                            self.file_size
                        ))
                        .safe_send("upload_file", &self.output_connection)
                        .await;
                        self.reset();
                        return;
                    }

                    if let Some(Some(metadata)) = metadata {
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
                    } else {
                        PPError::from("Failed to parse metadata info!")
                            .safe_send("upload_file", &self.output_connection)
                            .await;
                        self.reset();
                        return;
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
                match res {
                    Ok(_) => self.file_bytes_uploaded += content_fragment.len() as u64,
                    Err(err) => {
                        err.safe_send("upload_file", &self.output_connection).await;
                        self.reset();
                    }
                }
            }
        } 

        if self.file_size == self.file_bytes_uploaded {
            // If it's final segment, then finalize the FileUploader
            if let Some(fs_handler) = self.fs_handler.take() {
                let sha256_hash = fs_handler.finalize().await;
                // reset self
                self.reset();

                // Send ok
                self.output_connection
                    .write(
                        &MessageBuilder::build_from_str(
                            serde_json::to_string(&UploadFileResponse {
                                ok: true,
                                method: "upload_file".into(),
                                sha256_hash,
                            })
                            .unwrap(),
                        )
                        .packed(),
                    )
                    .await;
            }
        }
    }
}

impl FilesHandler {
    /// Resets everything besides `output_connection`
    pub fn reset(&mut self) {
        self.fs_handler = None;
        self.is_first = true;
        self.metadata_builder = None;
        self.file_bytes_uploaded = 0;
        self.file_size = 0;
        self.temp_buf = vec![];
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.output_connection.reader())
    }

    pub async fn new(connection: Arc<TCPConnection>) -> FilesHandler {
        FilesHandler {
            fs_handler: None,
            is_first: true,
            metadata_builder: None,
            file_size: 0,
            file_bytes_uploaded: 0,
            output_connection: connection,
            temp_buf: vec![],
        }
    }
}
