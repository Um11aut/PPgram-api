use log::debug;
use log::error;
use tokio::net::UdpSocket;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::io::AsyncReadExt;

use crate::db::bucket::DatabaseBucket;
use crate::db::bucket::DatabasePool;
use crate::server::message::Handler;
use crate::server::{message::handlers::tcp_handler::TCPHandler, session::Session};


const MESSAGE_ALLOCATION_SIZE: usize = 1024;

pub(super) type Sessions = Arc<RwLock<HashMap<i32, Arc<RwLock<Session>>>>>;

pub struct Server {
    tcp_listener: TcpListener,
    udp_socket: UdpSocket,
    connections: Sessions,
    pool: DatabasePool
}

impl Server {
    pub async fn new(udt_port: &str, tcp_port: &str) -> Option<Server> {
        let tcp_listener = TcpListener::bind(tcp_port).await.ok()?;
        let udp_socket = UdpSocket::bind(udt_port).await.ok()?;

        Some(Server {
            tcp_listener,
            udp_socket,
            connections: Arc::new(RwLock::new(HashMap::new())),
            pool: DatabasePool::new().await
        })
    }

    async fn tcp_event_handler(
        sessions: Sessions,
        session: Arc<RwLock<Session>>,
        bucket: DatabaseBucket,
        addr: SocketAddr,
    ) {
        debug!("TCP Connection established: {}", addr);

        let mut handler = TCPHandler::new(
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

        debug!("TCP Connection closed: {}", addr);
    }

    async fn udt_event_handler(
        sessions: Sessions,
        session: Arc<RwLock<Session>>,
        bucket: DatabaseBucket,
        addr: SocketAddr,
    ) {
        debug!("UDT Connection established: {}", addr);

        let mut handler = TCPHandler::new(
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

        debug!("UDT Connection closed: {}", addr);
    }

    pub async fn poll_events(self) {
        moro::async_scope!(|scope| {
            scope.spawn(async {
                Self::poll_tcp_events(self.tcp_listener, self.pool, self.connections).await;
            });

            scope.spawn(async {
                Self::poll_udt_events().await;
            });
        }).await;
    }

    async fn poll_tcp_events(listener: TcpListener, mut pool: DatabasePool, connections: Sessions) {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    let session = Arc::new(RwLock::new(Session::new(socket)));

                    let available_bucket = pool.get_available_bucket().await;
                    tokio::spawn(Self::tcp_event_handler(
                        connections.clone(),
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

    async fn poll_udt_events() {
        
    }
}
