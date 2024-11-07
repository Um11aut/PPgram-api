use std::{
    borrow::Borrow,
    net::{Ipv4Addr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};

use log::info;
use serde::Serialize;
use tokio::{net::tcp::OwnedReadHalf, sync::Mutex};
use webrtc::{sdp::MediaDescription, turn::server::request};

use crate::{
    db::internal::error::{PPError, PPResult},
    fs::{
        document::{DocumentFetcher, DocumentHandler},
        hash_exists,
        media::{is_media, MediaFetcher, MediaHandler},
        FsFetcher, FsUploader,
    },
    server::{
        connection::TCPConnection,
        message::{
            builder::MessageBuilder,
            types::{
                files::{
                    self, extract_media_method, DownloadFileMetadataResponse, DownloadFileRequest, FileMetadataRequest, Metadata
                },
                response::send::UploadFileResponse,
            },
            Handler,
        },
    },
};

use super::json_handler::SessionArcRwLock;

/// 4Gib - Max message size that can be transmitted
const MAX_MSG_SIZE: u64 = 4 * (1024 * 1024 * 1024 * 1024) /* Gib */;

struct FileFetcher {
    fs_handler: Box<dyn FsFetcher + Send + Sync>,
    /// Metadata of the file will be fetched in runtime
    metadata: Vec<Metadata>,
    bytes_fetched: u64,
}

impl FileFetcher {
    pub async fn new(sha256_hash: String) -> PPResult<Self> {
        let mut fs_handler: Box<dyn FsFetcher + Send + Sync> = if is_media(sha256_hash.as_str()).await? {
            Box::new(MediaFetcher::new(&sha256_hash))
        } else {
            Box::new(DocumentFetcher::new(&sha256_hash))
        };
        let metadata = fs_handler.fetch_metadata().await?;

        Ok(Self {
            fs_handler,
            metadata,
            bytes_fetched: 0,
        })
    }

    /// Fetch bytes part
    pub async fn fetch_data_frame(&mut self) -> PPResult<Vec<u8>> {
        Ok(self.fs_handler.fetch_part().await?)
    }

    /// Gets metadata
    /// 
    /// Dangerous: Metadata get's dynamic unloaded while calling `get_built_response` 
    pub fn get_metadata(&self) -> Vec<Metadata> {
        self.metadata.clone()
    }

    /// Gets ready response:
    ///
    /// If needed will add 8 bytes as the binary size for each binary
    pub async fn get_built_response(&mut self) -> PPResult<Vec<u8>> {
        let mut additional_data_frame = if self.fs_handler.is_part_ready() {
            if let Some(metadata) = self.metadata.first() {
                let size_as_be_bytes = metadata.file_size.to_be_bytes().to_vec();
                self.metadata.drain(..1);
                size_as_be_bytes
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let data_frame = self.fetch_data_frame().await?;
        additional_data_frame.extend(data_frame);

        Ok(additional_data_frame)
    }

    pub fn is_finished(&self) -> bool {
        self.metadata.is_empty()
    }
}

struct FileUploader {
    fs_handler: Box<dyn FsUploader + Send + Sync>,
    file_size: u64,
    bytes_uploaded: u64,
}

impl FileUploader {
    pub async fn new(metadata: FileMetadataRequest, file_size: u64) -> PPResult<Self> {
        if file_size > MAX_MSG_SIZE {
            return Err(PPError::from("Max. upload size exceeded!"));
        }

        Ok(Self {
            fs_handler: if metadata.is_media {
                Box::new(MediaHandler::new_uploader(metadata.name).await?)
            } else {
                Box::new(DocumentHandler::new_uploader(metadata.name).await?)
            },
            file_size,
            bytes_uploaded: 0,
        })
    }

    /// Upload file itself
    pub async fn consume_data_frame(&mut self, part: &[u8]) -> PPResult<()> {
        self.fs_handler.upload_part(part).await?;
        self.bytes_uploaded += part.len() as u64;

        Ok(())
    }

    pub async fn finalize(self) -> String {
        self.fs_handler.finalize().await
    }

    pub fn is_ready(&self) -> bool {
        self.file_size == self.bytes_uploaded
    }
}

/// To perform action - download or upload a file
enum FileActor {
    Uploader(FileUploader),
    Fetcher(FileFetcher),
}

/// TCP Handler to handle Files messages, meaning documents or media.
///
/// While uploading file,
/// The Metadata of the messages goes in JSON:
/// ```
/// [4 bytes metadata message size] [{"name": "Test123", "is_media": false, "compress": true}] [8 bytes file message size] [message bytes]
/// ```
pub struct FilesHandler {
    // to put the file frame on fs: Media or Document will be decided at runtime
    file_actor: Option<FileActor>,
    is_first: bool,
    // MessageBuilder for the first message
    request_builder: Option<MessageBuilder>,
    output_connection: Arc<TCPConnection>,
    // temp buffer if IP doesn't transfer all the bytes
    temp_buf: Vec<u8>,
}

#[async_trait::async_trait]
impl Handler for FilesHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        match self.handle(buffer).await {
            Ok(_) => {}
            Err(err) => {
                err.safe_send("upload", &self.output_connection).await;
                self.reset();
            }
        }
    }
}

impl FilesHandler {
    /// Resets everything besides `output_connection`
    pub fn reset(&mut self) {
        self.is_first = true;
        self.request_builder = None;
        self.temp_buf = vec![];
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.output_connection.reader())
    }

    async fn handle(&mut self, buffer: &[u8]) -> PPResult<()> {
        // For start message, it's not 0
        // needed to determine when the binary actually starts
        let mut content_start_offset: usize = 0;

        if self.is_first {
            self.request_builder = MessageBuilder::parse(&buffer);
            self.is_first = false;
        }

        // Then request isn't loaded yet
        if self.file_actor.is_none() {
            if let Some(request_builder) = self.request_builder.as_mut() {
                if request_builder.ready() {
                    // The offset of metadata message to separate metadata and content
                    let metadata_offset: usize = (request_builder.size() + 4).try_into().unwrap(); // 4 bytes for metadata message size
                    let content_start = &buffer[metadata_offset..];

                    let request_content = request_builder
                        .content_utf8()
                        .ok_or(PPError::from("Invalid UTF8 sequence transmitted!"))?;

                    let method = extract_media_method(&request_content)?;
                    match method.as_str() {
                        "upload_file" => {
                            info!("Uploading file chosen!");
                            let req =
                                serde_json::from_str::<FileMetadataRequest>(&request_content)?;

                            // If IP couldn't transfer enough bytes, extend the temp buffer
                            self.temp_buf.extend_from_slice(content_start);
                            if self.temp_buf.len() < 8 {
                                return Ok(());
                            }

                            // determine the file size of the next binary
                            let file_size = u64::from_be_bytes([
                                self.temp_buf[0],
                                self.temp_buf[1],
                                self.temp_buf[2],
                                self.temp_buf[3],
                                self.temp_buf[4],
                                self.temp_buf[5],
                                self.temp_buf[6],
                                self.temp_buf[7],
                            ]);

                            self.file_actor = Some(FileActor::Uploader(
                                FileUploader::new(req, file_size).await?,
                            ));
                        }
                        "download_file" => {
                            info!("Downloading file chosen!");
                            let req =
                                serde_json::from_str::<DownloadFileRequest>(&request_content)?;
                            self.file_actor =
                                Some(FileActor::Fetcher(FileFetcher::new(req.sha256_hash).await?));
                        }
                        _ => return Err(PPError::from("Invalid Method!")),
                    }

                    content_start_offset = metadata_offset + 8; // 8 bytes for files message size

                    request_builder.clear();
                    self.request_builder = None;
                }
            }
        }

        // file binary fragment
        let content_fragment = buffer[content_start_offset..].to_vec();
        // if !self.temp_buf.is_empty() {
        //     content_fragment.extend(&self.temp_buf);
        // }

        if let Some(file_actor) = self.file_actor.as_mut() {
            match file_actor {
                FileActor::Uploader(file_uploader) => {
                    file_uploader.consume_data_frame(&content_fragment).await?;

                    if file_uploader.is_ready() {
                        let actor = self.file_actor.take().unwrap();
                        match actor {
                            FileActor::Uploader(file_uploader) => {
                                let sha256_hash = file_uploader.finalize().await;

                                self.write_json(UploadFileResponse {
                                    ok: true,
                                    method: "upload_file".into(),
                                    sha256_hash,
                                }).await;
                            }
                            FileActor::Fetcher(_) => unimplemented!()
                        }
                    }
                }
                FileActor::Fetcher(file_fetcher) => {
                    self.output_connection
                        .write(
                            &MessageBuilder::build_from_str(
                                serde_json::to_string(&DownloadFileMetadataResponse{
                                    method: "download_file".into(),
                                    metadatas: file_fetcher.get_metadata()
                                })
                                .unwrap(),
                            )
                            .packed(),
                        )
                        .await;
                    
                    loop {
                        let data_frame = file_fetcher.get_built_response().await?;
                        self.output_connection.write(&data_frame).await;
    
                        if file_fetcher.is_finished() {
                            self.reset();
                            break;
                        }
                    }
                },
            }
        }

        Ok(())
    }

    async fn write_json(&self, value: impl Serialize) {
        self.output_connection
            .write(
                &MessageBuilder::build_from_str(
                    serde_json::to_string(&value)
                    .unwrap(),
                )
                .packed(),
            )
            .await;
    }
    
    pub async fn new(connection: Arc<TCPConnection>) -> FilesHandler {
        FilesHandler {
            file_actor: None,
            is_first: true,
            request_builder: None,
            output_connection: connection,
            temp_buf: vec![],
        }
    }
}
