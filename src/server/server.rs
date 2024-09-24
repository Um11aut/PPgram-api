use log::debug;
use log::error;
use std::collections::HashMap;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::io::AsyncReadExt;

use crate::db::connection::DatabaseBucket;
use crate::db::connection::DatabasePool;
use crate::server::{message::handler::Handler, session::Session};


const MESSAGE_ALLOCATION_SIZE: usize = 1024;

pub(super) type Sessions = Arc<RwLock<HashMap<i32, Arc<RwLock<Session>>>>>;

pub struct Server {
    listener: TcpListener,
    connections: Sessions,
    pool: DatabasePool
}

impl Server {
    pub async fn new(port: &str) -> Option<Server> {
        let listener = TcpListener::bind(port).await;

        if let Err(err) = listener {
            error!("Error while initializing the listener on the port: {}", err);
            return None;
        }

        Some(Server {
            listener: listener.unwrap(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            pool: DatabasePool::new().await
        })
    }

    async fn event_handler(
        sessions: Sessions,
        session: Arc<RwLock<Session>>,
        bucket: DatabaseBucket,
        addr: SocketAddr,
    ) {
        debug!("Connection established: {}", addr);

        let mut handler = Handler::new(
            Arc::clone(&session),
            Arc::clone(&sessions),
            bucket.clone()
        ).await;

        let reader = handler.reader();

        loop {
            let mut buffer = [0; MESSAGE_ALLOCATION_SIZE];

            match reader.lock().await.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    handler.handle_segmented_frame(&buffer[0..n]).await;
                }
                Err(_) => break,
            }
        }

        debug!("Connection closed: {}", addr);
    }

    pub async fn poll_events(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((socket, addr)) => {
                    let session = Arc::new(RwLock::new(Session::new(socket)));

                    let available_bucket = self.pool.get_available_bucket().await;
                    tokio::spawn(Self::event_handler(
                        Arc::clone(&self.connections),
                        Arc::clone(&session),
                        available_bucket,
                        addr,
                    ));
                }
                Err(err) => {
                    error!("Error while establishing new connection: {}", err);
                }
            }
        }
    }
}
