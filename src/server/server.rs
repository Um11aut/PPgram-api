use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpSocket, TcpStream}};
use std::{collections::VecDeque, net::SocketAddr, sync::Arc};
use log::{debug, error, info, trace};
use std::future::Future;

use crate::server::{message::{self, message::RequestMessage, handler::RequestMessageHandler}, session::Session};

const PACKET_SIZE: u32 = 65000;

pub(crate) struct Server {
    listener: TcpListener
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
                listener: listener.unwrap()
            }
        )
    }

    pub async fn listen(&mut self)
    {
        loop {
            let res = self.listener.accept().await;

            if let Err(err) = res {
                error!("Error while establishing new connection: {}", err);
                continue;
            }

            let (mut socket, addr) = res.unwrap();
            
            tokio::spawn(async move {
                debug!("Connection established: {}", addr);

                let mut session = Session::new();
                let mut handler = RequestMessageHandler::new();

                loop {
                    let mut buffer = [0; PACKET_SIZE as usize];
                    match socket.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if !session.is_authenticated() 
                            {
                                handler.handle_authentication(&buffer[0..n], &mut socket, &mut session).await;
                                continue;
                            }
                            
                            handler.handle_segmented_frame(&buffer[0..n], &mut socket).await;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                debug!("Connection closed. {}", addr);
            });
        }
    }
}