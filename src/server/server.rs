use log::debug;
use log::error;
use log::info;
use quinn::crypto::rustls::QuicServerConfig;
use quinn::rustls;
use quinn::rustls::pki_types::PrivatePkcs8KeyDer;
use std::collections::HashMap;
use std::fmt::format;
use std::net::Ipv4Addr;
use std::net::SocketAddrV4;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::{net::SocketAddr, sync::Arc};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use crate::db::bucket;
use crate::db::bucket::DatabaseBucket;
use crate::db::bucket::DatabasePool;
use crate::db::internal::error::PPResult;
use crate::server::message::Handler;
use crate::server::{message::handlers::tcp_handler::TCPHandler, session::Session};

use super::connection;
use super::message::handlers::tcp_handler::SesssionArcRwLock;

/// Where RSA key will be stored in FS
const RSA_KEY_BASE: &str = "/server_data/keys";

const MESSAGE_ALLOCATION_SIZE: usize = 1024;

pub(super) type Sessions = Arc<RwLock<HashMap<i32, Arc<RwLock<Session>>>>>;

pub struct Server {
    tcp_listener: TcpListener,
    quic_endpoint: quinn::Endpoint,
    connections: Sessions,
    pool: DatabasePool,
}

impl Server {
    pub async fn new(quic_port: u16, tcp_port: u16) -> PPResult<Server> {
        let tcp_listener = TcpListener::bind(format!("0.0.0.0:{}", tcp_port)).await?;

        // generate random RSA key
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "*".into()]).unwrap();
        let key: PrivatePkcs8KeyDer<'_> = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.cert.der().to_owned()], key.into())?;
        server_crypto.alpn_protocols = [b"hq-29"].iter().map(|&v| v.into()).collect();

        let mut server_config =
            quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_crypto)?));
        
        let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
        // Allow only Bidirectional streams
        transport_config.max_concurrent_uni_streams(0_u8.into());
        
        let server: quinn::Endpoint = quinn::Endpoint::server(
            server_config,
            SocketAddr::parse_ascii(format!("0.0.0.0:{}", quic_port).as_bytes()).unwrap()
        )?;

        info!("[QUIC] listening on port: {}", server.local_addr().unwrap());
        info!("[TCP] listening on port: {}", tcp_port);

        Ok(Server {
            tcp_listener,
            quic_endpoint: server,
            connections: Arc::new(RwLock::new(HashMap::new())),
            pool: DatabasePool::new().await,
        })
    }

    async fn tcp_event_handler(
        socket: TcpStream,
        addr: SocketAddr,
        sessions: Sessions,
        bucket: DatabaseBucket,
    ) {
        debug!("TCP Connection established: {}", addr);
        let session = Arc::new(RwLock::new(Session::new(socket)));

        let mut handler =
            TCPHandler::new(Arc::clone(&session), Arc::clone(&sessions), bucket).await;

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

    async fn quic_event_handler(con: quinn::Incoming, bucket: DatabaseBucket) {
        let connection = con.await;

        match connection {
            Ok(connection) => {
                info!("QUIC connection established from: {}", connection.remote_address());
                let stream = connection.accept_bi().await;
                match stream {
                    Ok((mut send, mut recv)) => {
                        info!("Stream accepted, waiting for data...");
                        let req = recv.read_to_end(64 * 1024).await.unwrap();
                        info!("{:?}", req);
                        send.write(&[1,2,3,4]).await.unwrap();
                    }
                    Err(err) => error!("Failed accepting stream: {}", err),
                }
            }
            Err(err) => error!("Error while establishing connection: {}", err),
        }
    }

    pub async fn poll_events(self) {
        let pool = Arc::new(Mutex::new(self.pool));
        moro::async_scope!(|scope| {
            scope.spawn(async {
                Self::poll_tcp_events(
                    self.tcp_listener,
                    Arc::clone(&pool),
                    Arc::clone(&self.connections),
                )
                .await;
            });

            scope.spawn(async {
                Self::poll_quic_events(
                    self.quic_endpoint,
                    Arc::clone(&pool),
                    Arc::clone(&self.connections),
                )
                .await;
            });
        })
        .await;
    }

    async fn poll_tcp_events(
        listener: TcpListener,
        pool: Arc<Mutex<DatabasePool>>,
        connections: Sessions,
    ) {
        info!("[TCP] Listening for connections...");
        moro::async_scope!(|scope| {
            loop {
                match listener.accept().await {
                    Ok((socket, addr)) => {
                        let available_bucket = {
                            let mut db_pool = pool.lock().await;
                            db_pool.get_available_bucket().await
                        };

                        scope.spawn(Self::tcp_event_handler(
                            socket,
                            addr,
                            Arc::clone(&connections),
                            available_bucket,
                        ));
                    }
                    Err(err) => {
                        error!("Error while establishing new connection: {}", err);
                    }
                }
            }
        })
        .await;
    }

    async fn poll_quic_events(
        endpoint: quinn::Endpoint,
        pool: Arc<Mutex<DatabasePool>>,
        connections: Sessions,
    ) {
        while let Some(con) = endpoint.accept().await {
            info!("[QUIC] Got Connection candidate: {:?}", con.remote_address());
            let available_bucket = {
                let mut db_pool = pool.lock().await;
                db_pool.get_available_bucket().await
            };

            tokio::spawn(async move {
                Self::quic_event_handler(con, available_bucket)
            });
        }
    }
}
