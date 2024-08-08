use log::{debug, error, info, trace};
use serde_json::json;
use std::{future::Future, net::IpAddr};
use tokio::sync::RwLock;
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use tokio::net::tcp::WriteHalf;
use tokio::sync::mpsc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf},
        TcpListener, TcpSocket, TcpStream,
    },
    sync::Mutex,
};

use crate::server::{
    message::{
        self, builder::Message, handler::RequestMessageHandler, types::message::RequestMessage,
    },
    session::Session,
};

const PACKET_SIZE: u32 = 65535;

pub(crate) struct Server {
    listener: TcpListener,
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<Session>>>>>,
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
        connections: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<Session>>>>>,
        session: Arc<Mutex<Session>>,
        addr: SocketAddr,
    ) {
        debug!("Connection established: {}", addr);

        for (ip, s) in connections.read().await.iter() {
            {
                let s = s.lock().await;

                if let Some(s) = s.get_credentials() {
                    info!("ip: {}. [user_id: {}, session_id: {}]", ip, s.0, s.1);
                }
            }
            
            if *ip != addr {
                if *s.lock().await != *session.lock().await {
                    let message_builder = Message::build_from("Hello!");
                    s.lock().await.send(message_builder.packed()).await;
                }
            }
        }

        let handler = Arc::new(Mutex::new(RequestMessageHandler::new(
            Arc::clone(&writer),
            Arc::clone(&session),
        )));

        loop {
            let mut buffer = [0; PACKET_SIZE as usize];

            match reader.lock().await.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    tokio::spawn({
                        let handler = Arc::clone(&handler);
                        async move {
                            let mut handler = handler.lock().await;
                            handler.handle_segmented_frame(&buffer[0..n]).await;
                        }
                    });
                }
                Err(_) => break,
            }
        }

        {
            let mut connections = connections.write().await;
            connections.remove(&addr);
        }

        debug!("Connection closed: {}", addr);
    }

    async fn receiver_handler(mut rx: mpsc::Receiver<String>, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let writer = Arc::clone(&writer);

        while let Some(message) = rx.recv().await {
            let mut writer = writer.lock().await;
            if let Err(e) = writer.write_all(message.as_bytes()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }

    pub async fn listen(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((socket, addr)) => {
                    let (sender, receiver) = mpsc::channel::<String>(PACKET_SIZE as usize);

                    let session = Arc::new(Mutex::new(Session::new(addr, sender)));

                    {
                        self.connections.write().await.insert(addr, Arc::clone(&session));
                    }

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
