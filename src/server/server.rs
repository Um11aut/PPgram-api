use serde_json::json;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpSocket, TcpStream}};
use std::{collections::VecDeque, net::SocketAddr, sync::Arc};
use log::{debug, error, info, trace};
use std::future::Future;

use crate::server::{message::{self, RequestMessage}, session::Session};

const PACKET_SIZE: u32 = 1024;

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

                let mut temp_buffer: Vec<u8> = Vec::new();
                let mut parts: VecDeque<u32> = VecDeque::new();
                let mut is_header = false;


                let mut message_size: Option<_> = None;

                loop {
                    let mut buffer = [0; 1024];
                    match socket.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if !session.is_authenticated() {
                                continue;
                            }
                            
                            let mut message = std::str::from_utf8(&buffer[0..n]).unwrap();

                            let mut pos: Option<_> = None;
                            let mut rest: Option<&str> = None;
                            if message.starts_with("size") {
                                let re = regex::Regex::new(r"size=(\d+)").unwrap();

                                if let Some(caps) = re.captures(message) {
                                    if let Ok(size) = caps[1].parse::<u32>() {
                                        message_size = Some(size);
                                    }
                                }

                                is_header = true;
                                pos = Some(message.find("\\n").unwrap());
                                rest = Some(&message[0..pos.unwrap()+2]);
                                message = &message[pos.unwrap() + 2..];

                                if let Some(message_size) = message_size {
                                    info!("Got the header message: {}", message_size);
                                    
                                    temp_buffer = Vec::with_capacity(1024);
                                    
                                    let mut remaining = message_size;
                                    
                                    while remaining > 0 {
                                        let chunk = if remaining > PACKET_SIZE {
                                            PACKET_SIZE
                                        } else {
                                            remaining
                                        };
                                        parts.push_back(chunk);
                                        remaining -= chunk;
                                    }

                                    if parts[0] == PACKET_SIZE {
                                        parts[0] -= rest.unwrap().len() as u32;
                                        parts[1] += rest.unwrap().len() as u32;
                                    }
                                }
                            }

                            if parts.is_empty() && is_header {
                                continue;
                            }

                            if *parts.front().unwrap() != message.len() as u32 {
                                error!("Invalid parts: {} != {}", message.len(), *parts.front().unwrap());
                                continue;
                            }

                            temp_buffer.extend_from_slice(message.as_bytes());

                            if temp_buffer.len() == message_size.unwrap() as usize {
                                let message = String::from_utf8_lossy(&temp_buffer).into_owned();
                                let res: Result<RequestMessage, _> = serde_json::from_str(message.as_str());

                                if let Err(err) = res {
                                    error!("{}", err);
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

                                temp_buffer.clear();
                            } else {
                                info!("{}, {}", temp_buffer.len(), parts.iter().sum::<u32>());
                            }

                            parts.pop_front();
                        }
                        // if socket.write_all(&buffer[0..n]).await.is_err() {
                        //     break;
                        // }
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