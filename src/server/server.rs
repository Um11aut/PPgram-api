use log::debug;
use log::error;
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

use crate::server::{message::handler::RequestMessageHandler, session::Session};

use super::message::builder::Message;

const PACKET_SIZE: u32 = 65535;

pub(super) type Connections = Arc<RwLock<HashMap<i32, Arc<Mutex<Session>>>>>;

pub(crate) struct Server {
    listener: TcpListener,
    connections: Connections,
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
        reader: Arc<Mutex<OwnedReadHalf>>,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        connections: Connections,
        session: Arc<Mutex<Session>>,
        addr: SocketAddr,
    ) {
        debug!("Connection established: {}", addr);

        let mut handler = RequestMessageHandler::new(
            Arc::clone(&writer),
            Arc::clone(&session),
            Arc::clone(&connections),
        );

        loop {
            let mut buffer = [0; PACKET_SIZE as usize];

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

    async fn receiver_handler(mut rx: mpsc::Receiver<String>, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let writer = Arc::clone(&writer);

        while let Some(message) = rx.recv().await {
            let mut writer = writer.lock().await;
            if let Err(e) = writer.write_all(Message::build_from(message).packed().as_bytes()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }

    pub async fn poll_events(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((socket, addr)) => {
                    let (sender, receiver) = mpsc::channel::<String>(PACKET_SIZE as usize);

                    let session = Arc::new(Mutex::new(Session::new(addr, sender)));

                    let (reader, writer) = {
                        let (r, w) = socket.into_split();

                        (Arc::new(Mutex::new(r)), Arc::new(Mutex::new(w)))
                    };

                    tokio::spawn(Self::event_handler(
                        Arc::clone(&reader),
                        Arc::clone(&writer),
                        Arc::clone(&self.connections),
                        Arc::clone(&session),
                        addr,
                    ));

                    tokio::spawn(Self::receiver_handler(receiver, writer));
                }
                Err(err) => {
                    error!("Error while establishing new connection: {}", err);
                }
            }
        }
    }
}
