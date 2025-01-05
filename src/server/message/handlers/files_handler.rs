use std::sync::Arc;

use log::{debug, error, info, trace};
use tokio::{net::tcp::OwnedReadHalf, sync::Mutex};

use crate::{
    db::{
        bucket::{self, DatabaseBucket, DatabaseBuilder},
        internal::error::{PPError, PPResult},
    },
    fs::{
        document::fetch_hash_metadata,
        helpers::{
            fetcher::{FileFetcher, MediaFetchMode},
            uploader::FileUploader,
        },
    },
    server::{
        connection::TCPConnection,
        message::{
            builder::MessageBuilder,
            types::{
                files::{
                    extract_file_method, DownloadFileMetadataResponse, DownloadFileRequest,
                    DownloadMetadataRequest, FileMetadataRequest,
                },
                response::send::UploadFileResponse,
            },
            Handler,
        },
    },
};

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
    bucket: DatabaseBucket,
    // to put/download the file frame on fs: Media or Document will be decided at runtime
    file_actor: Option<FileActor>,
    is_first: bool,
    // MessageBuilder for the first message
    request_builder: Option<MessageBuilder>,
    output_connection: Arc<TCPConnection>,
    // temp buffer if IP doesn't transfer all the bytes
    content_buf: Vec<u8>,
    // accumulated if IP doesn't transfer all the bytes
    accumulated_binary_start: Vec<u8>,
}

#[async_trait::async_trait]
impl Handler for FilesHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        match self.handle(buffer).await {
            Ok(_) => {}
            Err(err) => {
                error!("[Files] Error while performing file operation:\n {}", err);
                err.safe_send("file_operation", &self.output_connection)
                    .await;
                self.reset();
            }
        }
    }
}

macro_rules! write_json {
    ($connection:expr, $value:expr) => {{
        $connection
            .write(
                &MessageBuilder::build_from_str(serde_json::to_string(&$value).unwrap()).packed(),
            )
            .await;
    }};
}

impl FilesHandler {
    /// Resets everything besides `output_connection`
    pub fn reset(&mut self) {
        self.file_actor = None;
        self.is_first = true;
        self.request_builder = None;
        self.content_buf = vec![];
        self.accumulated_binary_start = vec![];
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.output_connection.reader())
    }

    async fn handle(&mut self, buffer: &[u8]) -> PPResult<()> {
        let mut do_extend = true;
        trace!("{}", buffer.len());
        if self.is_first {
            self.request_builder = MessageBuilder::parse(buffer);
        }

        // Then request isn't loaded yet
        if self.file_actor.is_none() {
            if let Some(request_builder) = self.request_builder.as_mut() {
                if request_builder.ready() {
                    // The offset of metadata message to separate metadata and content
                    let metadata_offset: usize = if self.is_first {
                        (request_builder.size() + 4).try_into().unwrap()
                    } else {
                        0
                    }; // 4 bytes for metadata message size

                    let request_content = request_builder
                        .content_utf8()
                        .ok_or(PPError::from("Invalid UTF8 sequence transmitted!"))?;

                    if self.is_first {
                        debug!(
                            "[Files] Got File Message!\n Message Size: {}\n Message Content: {}",
                            request_content.len(),
                            request_content
                        );
                    }

                    let method = extract_file_method(request_content)?;
                    match method.as_str() {
                        "upload_file" => {
                            let content_start = &buffer[metadata_offset..];
                            if content_start.is_empty() {
                                self.is_first = false;
                                return Ok(());
                            }
                            let req: FileMetadataRequest = serde_json::from_str(request_content)?;

                            self.accumulated_binary_start
                                .extend_from_slice(content_start);
                            if self.accumulated_binary_start.len() < 8 {
                                self.is_first = false;
                                return Ok(());
                            }

                            // determine the file size of the next binary
                            let file_size = u64::from_be_bytes([
                                self.accumulated_binary_start[0],
                                self.accumulated_binary_start[1],
                                self.accumulated_binary_start[2],
                                self.accumulated_binary_start[3],
                                self.accumulated_binary_start[4],
                                self.accumulated_binary_start[5],
                                self.accumulated_binary_start[6],
                                self.accumulated_binary_start[7],
                            ]);
                            // Drain the binary size
                            self.accumulated_binary_start.drain(..8);

                            // extending content buffer and clearing accumulated binary start
                            self.content_buf
                                .extend(self.accumulated_binary_start.clone());
                            self.accumulated_binary_start.clear();

                            self.file_actor = Some(FileActor::Uploader(
                                FileUploader::new(req, file_size).await?,
                            ));

                            request_builder.clear();
                            self.is_first = false;
                            do_extend = false;
                        }
                        "download_file" => {
                            let req: DownloadFileRequest = serde_json::from_str(request_content)?;
                            self.file_actor = Some(FileActor::Fetcher(
                                FileFetcher::new(
                                    DatabaseBuilder::from(self.bucket.clone()).into(),
                                    req.sha256_hash,
                                    MediaFetchMode::try_from(req.mode.as_str())?,
                                )
                                .await?,
                            ));
                        }
                        "download_metadata" => {
                            let req: DownloadMetadataRequest =
                                serde_json::from_str(request_content)?;
                            let (main_metadata, maybe_metadata) =
                                fetch_hash_metadata(self.get_db(), &req.sha256_hash).await?;

                            write_json!(
                                &self.output_connection,
                                DownloadFileMetadataResponse {
                                    ok: true,
                                    method: "download_metadata".into(),
                                    file_metadata: Some(main_metadata),
                                    preview_metadata: maybe_metadata,
                                }
                            );
                            self.reset();

                            return Ok(());
                        }
                        _ => return Err("Unknown method provided".into()),
                    }

                    request_builder.clear();
                    self.is_first = false;
                }
            }
        }

        if do_extend {
            self.content_buf.extend_from_slice(buffer);
        }

        if let Some(file_actor) = self.file_actor.as_mut() {
            match file_actor {
                FileActor::Uploader(file_uploader) => {
                    trace!(
                        "[Upload] Uploading datagramm. Size: {}",
                        self.content_buf.len()
                    );

                    let content_fragment = &self.content_buf;
                    if content_fragment.is_empty() {
                        return Ok(());
                    }

                    let rest_to_upload = file_uploader.rest_to_upload() as usize;
                    if content_fragment.len() > rest_to_upload {
                        info!("Transmitted more than one binary at once!");
                        // after this fragment, it's not a part of current binary anymore
                        let to_upload =
                            &self.content_buf[..content_fragment.len() - rest_to_upload];
                        file_uploader.consume_data_frame(to_upload).await?;
                        self.content_buf
                            .drain(..content_fragment.len() - rest_to_upload);
                    } else {
                        file_uploader.consume_data_frame(content_fragment).await?;
                        self.content_buf.clear();
                    }

                    if file_uploader.is_ready() {
                        let actor = self.file_actor.take().unwrap();
                        if let FileActor::Uploader(file_uploader) = actor {
                            let sha256_hash = file_uploader.finalize(&self.get_db()).await?;

                            write_json!(
                                &self.output_connection,
                                UploadFileResponse {
                                    ok: true,
                                    method: "upload_file".into(),
                                    sha256_hash,
                                }
                            );

                            self.reset();
                        }
                    }
                }
                FileActor::Fetcher(file_fetcher) => {
                    let (maybe_main, maybe_preview) = file_fetcher.get_metadata();
                    write_json!(
                        &self.output_connection,
                        DownloadFileMetadataResponse {
                            ok: true,
                            method: "download_file".into(),
                            file_metadata: maybe_main,
                            preview_metadata: maybe_preview
                        }
                    );

                    loop {
                        // As we have Metadatas, we may not include the size of the next binary frame
                        let data_frame = file_fetcher.fetch_data_frame().await?;
                        if data_frame.is_empty() {
                            self.reset();
                            return Ok(());
                        }
                        trace!(
                            "[Download] Sending data frame back! Data Frame size: {}",
                            data_frame.len()
                        );
                        self.output_connection.write(data_frame).await;

                        if file_fetcher.is_finished() {
                            self.reset();
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // Function to get any database by just passing the type
    #[inline]
    fn get_db<T: From<DatabaseBuilder>>(&self) -> T {
        DatabaseBuilder::from(self.bucket.clone()).into()
    }

    pub async fn new(connection: Arc<TCPConnection>, bucket: DatabaseBucket) -> FilesHandler {
        FilesHandler {
            bucket,
            file_actor: None,
            is_first: true,
            request_builder: None,
            output_connection: connection,
            content_buf: vec![],
            accumulated_binary_start: vec![],
        }
    }
}
