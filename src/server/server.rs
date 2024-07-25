use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf}, TcpListener, TcpSocket, TcpStream}, sync::Mutex};
use std::{collections::{HashMap, VecDeque}, net::SocketAddr, ops::{Deref, DerefMut}, sync::Arc};
use log::{debug, error, info, trace};
use std::future::Future;
use tokio::sync::mpsc;
use tokio::net::tcp::WriteHalf;

use crate::server::{message::{self, builder::Message, handler::RequestMessageHandler, message::RequestMessage}, session::Session};

const PACKET_SIZE: u32 = 65535;

pub(crate) struct Server {
    listener: TcpListener,
    connections: Arc<Mutex<HashMap<Session, mpsc::Sender<String>>>>
}

impl Server {
    pub async fn new(port: &str) -> Option<Server> {
        let listener = TcpListener::bind(port).await;

        if let Err(err) = listener {
            error!("Error while initializing the listener on the port: {}", err);
            return None
        }
        
        Some(
            Server {
                listener: listener.unwrap(),
                connections: Arc::new(Mutex::new(HashMap::new()))
            }
        )
    }

    async fn handle_connection(
        reader: Arc<Mutex<OwnedReadHalf>>,
        writer: Arc<Mutex<OwnedWriteHalf>>,
        connections: Arc<Mutex<HashMap<Session, mpsc::Sender<String>>>>,
        mut session: Session,
        addr: SocketAddr,
    ) {
        debug!("Connection established: {}", addr);
        
        for (s, tx) in connections.lock().await.iter() {
            if *s != session {
                let message_builder = Message::build_from("Hello!");
                tx.send(message_builder.packed()).await.unwrap();
            }
        }

        let mut handler = RequestMessageHandler::new();
        
        loop {
            let mut buffer = [0; PACKET_SIZE as usize];

            let mut reader = reader.lock().await;

            match reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    let mut writer = writer.lock().await;
                    if !session.is_authenticated() {
                        handler
                            .handle_authentication(&buffer[0..n], &mut writer, &mut session)
                            .await;
                        continue;
                    }
    
                    handler.handle_segmented_frame(&buffer[0..n], &mut writer).await;
                }
                Err(_) => break,
            }
        }
    
        {
            let mut connections = connections.lock().await;
            connections.remove(&session);
        }
    
        debug!("Connection closed: {}", addr);
    }

    async fn handle_outgoing_messages(mut rx: mpsc::Receiver<String>, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let writer = Arc::clone(&writer);

        while let Some(message) = rx.recv().await {
            let mut writer = writer.lock().await;
            if let Err(e) = writer.write_all(message.as_bytes()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }

    pub async fn listen(&mut self)
    {
        loop {
            match self.listener.accept().await {
                Ok((socket, addr)) => {
                    let (tx, rx) = mpsc::channel::<String>(1000);
                    
                    let session = Session::new(addr);

                    {
                        self.connections.lock().await.insert(session.clone(), tx);
                    }

                    let (reader, writer) = {
                        let (r, w) = socket.into_split();

                        (Arc::new(Mutex::new(r)), Arc::new(Mutex::new(w)))
                    };

                    tokio::spawn(Self::handle_connection(
                        Arc::clone(&reader),
                        Arc::clone(&writer),
                        Arc::clone(&self.connections),
                        session,
                        addr,
                    ));

                    tokio::spawn(Self::handle_outgoing_messages(
                        rx,
                        writer
                    ));
                }
                Err(err) => {
                    error!("Error while establishing new connection: {}", err);
                }
            }
        }
    }
}