use log::debug;
use log::error;
use log::info;
use serde_json::Value;
use std::collections::hash_map;
use std::collections::HashMap;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::Mutex,
};

use crate::server::connection;
use crate::server::{message::handler::MessageHandler, session::Session};

use super::message::builder::MessageBuilder;

const MESSAGE_ALLOCATION_SIZE: usize = 1024;

pub(super) type Sessions = Arc<RwLock<HashMap<i32, Arc<RwLock<Session>>>>>;

pub(crate) struct Server {
    listener: TcpListener,
    connections: Sessions,
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
        })
    }

    async fn event_handler(
        sessions: Sessions,
        session: Arc<RwLock<Session>>,
        addr: SocketAddr,
    ) {
        debug!("Connection established: {}", addr);

        let session_locked = session.read().await;
        let reader = Arc::clone(&session_locked.connections[0].reader);
        drop(session_locked);

        let mut handler = MessageHandler::new(
            Arc::clone(&session),
            Arc::clone(&sessions),
            0
        );

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

                    tokio::spawn(Self::event_handler(
                        Arc::clone(&self.connections),
                        Arc::clone(&session),
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
