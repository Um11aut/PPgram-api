use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpSocket, TcpStream}};
use std::{net::SocketAddr, sync::Arc};
use log::{debug, error, info, trace};
use std::future::Future;

use crate::server::{message::RequestMessage, session::Session};

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

                let session: Session = Session::new(addr);
                let mut buffer = [0; 1024];

                loop {
                    match socket.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if session.is_authenticated() {
                                println!("{}", std::str::from_utf8(&buffer).unwrap());
                                let res: Result<RequestMessage, _> = serde_json::from_str(std::str::from_utf8(&buffer).unwrap());

                                if let Err(_) = res {
                                    let data = json!({
                                        "ok": false
                                    });
                                    if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                                        break;
                                    }
                                } else {
                                    let data = json!({
                                        "ok": true
                                    });
                                    if socket.write_all(serde_json::to_string(&data).unwrap().as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            // if socket.write_all(&buffer[0..n]).await.is_err() {
                            //     break;
                            // }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                debug!("Connection closed.{}", addr);
            });
        }
    }
}