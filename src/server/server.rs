use dashmap::DashMap;
use log::debug;
use log::error;
use log::info;
use std::{net::SocketAddr, sync::Arc};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use crate::db::bucket::DatabaseBucket;
use crate::db::bucket::DatabasePool;
use crate::db::internal::error::PPResult;
use crate::server::connection::TCPConnection;
use crate::server::message::handlers::files_handler::FilesHandler;
use crate::server::message::Handler;
use crate::server::{message::handlers::json_handler::JsonHandler, session::Session};

use super::message::handlers::json_handler::SessionArcRwLock;

/// 1024 bytes
const JSON_MESSAGE_ALLOCATION_SIZE: usize = 1024;

/// 1 Mib
pub const FILES_MESSAGE_ALLOCATION_SIZE: usize = 1024 * 1024;

pub(super) type Sessions = Arc<DashMap<i32, SessionArcRwLock>>;

/// Two ports are available:
/// 3000 - For Json Messages. The full message is stored in a `Vec`(on RAM) and handled after they are completely received
/// 8080 - For Files Messages. The file message consists of metadata and the binary itself. After the metadata is sended, goes
/// the binary itself.
pub struct Server {
    json_listener: TcpListener,
    file_listener: TcpListener,
    connections: Sessions,
    pool: DatabasePool,
}

impl Server {
    pub async fn new(json_port: u16, files_port: u16) -> PPResult<Server> {
        let json_listener = TcpListener::bind(format!("0.0.0.0:{}", json_port)).await?;
        let file_listener = TcpListener::bind(format!("0.0.0.0:{}", files_port)).await?;

        info!("[JSON Messages] listening on port: {}", json_port);
        info!("[Files Messages] listening on port: {}", files_port);

        Ok(Server {
            json_listener,
            file_listener,
            connections: Arc::new(DashMap::new()),
            pool: DatabasePool::new().await,
        })
    }

    /// Handle all Json Messages
    async fn json_event_handler(
        socket: TcpStream,
        addr: SocketAddr,
        sessions: Sessions,
        bucket: DatabaseBucket,
    ) {
        debug!("[JSON] Connection established: {}", addr);
        let session = Arc::new(RwLock::new(Session::new(socket)));

        let mut handler =
            JsonHandler::new(Arc::clone(&session), Arc::clone(&sessions), bucket).await;

        let reader = handler.reader();
        let mut buffer = Box::new([0; JSON_MESSAGE_ALLOCATION_SIZE]);

        loop {
            match reader.lock().await.read(&mut buffer[..]).await {
                Ok(0) => break,
                Ok(n) => {
                    handler.handle_segmented_frame(&buffer[0..n]).await;
                }
                Err(_) => break,
            }
        }

        debug!("[JSON] Connection closed: {}", addr);
    }

    async fn files_event_handler(
        socket: TcpStream,
        addr: SocketAddr,
        _sessions: Sessions,
        bucket: DatabaseBucket,
    ) {
        debug!("[Files] Connection established: {}", addr);
        let mut handler = FilesHandler::new(Arc::new(TCPConnection::new(socket)), bucket).await;

        let reader = handler.reader();

        // Store buffer on the heap to avoid StackOverflow
        let mut buffer = Vec::new();
        buffer.resize(FILES_MESSAGE_ALLOCATION_SIZE, Default::default());

        loop {
            match reader.lock().await.read(&mut buffer).await {
                Ok(0) => {
                    drop(buffer);
                    break;
                }
                Ok(n) => {
                    handler.handle_segmented_frame(&buffer[0..n]).await;
                }
                Err(_) => {
                    drop(buffer);
                    break;
                }
            }
        }

        debug!("[Files] Connection closed: {}", addr);
    }

    /// asynchronously starts JSON and Files TCP servers
    pub async fn poll_events(self) {
        let pool = Arc::new(Mutex::new(self.pool));
        moro::async_scope!(|scope| {
            scope.spawn(async {
                Self::poll_json_events(
                    self.json_listener,
                    Arc::clone(&pool),
                    Arc::clone(&self.connections),
                )
                .await;
            });

            scope.spawn(async {
                Self::poll_files_events(
                    self.file_listener,
                    Arc::clone(&pool),
                    Arc::clone(&self.connections),
                )
                .await;
            });
        })
        .await;
    }

    async fn poll_json_events(
        listener: TcpListener,
        pool: Arc<Mutex<DatabasePool>>,
        connections: Sessions,
    ) {
        moro::async_scope!(|scope| {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let available_bucket = {
                            let mut db_pool = pool.lock().await;
                            db_pool.get_available_bucket().await
                        };

                        scope.spawn(Self::json_event_handler(
                            socket,
                            addr,
                            Arc::clone(&connections),
                            available_bucket,
                        ));
                    }
                    Err(err) => {
                        error!("Error while establishing new JSON connection: {}", err);
                    }
                }
            }
        })
        .await;
    }

    async fn poll_files_events(
        listener: TcpListener,
        pool: Arc<Mutex<DatabasePool>>,
        connections: Sessions,
    ) {
        // TODO: Make sure that user has access rights to the hash
        moro::async_scope!(|scope| {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let available_bucket = {
                            let mut db_pool = pool.lock().await;
                            db_pool.get_available_bucket().await
                        };

                        scope.spawn(Self::files_event_handler(
                            socket,
                            addr,
                            Arc::clone(&connections),
                            available_bucket,
                        ));
                    }
                    Err(err) => {
                        error!("Error while establishing new Files connection: {}", err);
                    }
                }
            }
        })
        .await;
    }
}
