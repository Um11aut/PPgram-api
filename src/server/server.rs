use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf}, TcpListener, TcpSocket, TcpStream}, sync::Mutex};
use std::{collections::{HashMap, VecDeque}, net::SocketAddr, ops::{Deref, DerefMut}, sync::Arc};
use log::{debug, error, info, trace};
use std::future::Future;
use tokio::sync::mpsc;
use tokio::net::tcp::WriteHalf;

use crate::server::{message::{self, message::RequestMessage, handler::RequestMessageHandler}, session::Session};

const PACKET_SIZE: u32 = 65000;

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
                tx.send(String::from("Hello!")).await.unwrap();
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

            tokio::task::yield_now().await;
        }
    
        {
            let mut connections = connections.lock().await;
            connections.remove(&session);
        }
    
        debug!("Connection closed: {}", addr);
    }

    pub async fn listen(&mut self)
    {
        loop {
            let res = self.listener.accept().await;

            if let Err(err) = res {
                error!("Error while establishing new connection: {}", err);
                continue;
            }

            let (tx, mut rx) = mpsc::channel::<String>(1000);
            let (socket, addr) = res.unwrap();
            let session = Session::new(addr);

            {
                let mut connections = self.connections.lock().await;
                connections.insert(session.clone(), tx);
            }

            let (reader, writer) = socket.into_split();
            let reader = Arc::new(Mutex::new(reader));
            let writer = Arc::new(Mutex::new(writer));

            tokio::spawn(Self::handle_connection(
                Arc::clone(&reader),
                Arc::clone(&writer),
                Arc::clone(&self.connections),
                session,
                addr,
            ));

            tokio::spawn({
                let (_, writer) = {(Arc::clone(&reader), Arc::clone(&writer))};

                async move {
                    while let Some(message) = rx.recv().await {
                        let mut writer = writer.lock().await;
                        if let Err(e) = writer.write_all(message.as_bytes()).await {
                            error!("Failed to send message: {}", e);
                        }
                    }
                }
            });
        }
    }
}