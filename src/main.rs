pub mod server;
use std::{net::SocketAddr, sync::Arc};

use server::message::RequestMessage;
use log::error;
use log::info;
use log::warn;
use log::LevelFilter;
use server::server::Server;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

async fn handle_connection(stream: &mut TcpStream, addr: SocketAddr) {
    let mut buf = [0; 1024];

    match stream.read(&mut buf).await {
        Ok(n) if n == 0 => return,
        Ok(n) => {
            if let Err(e) = stream.write_all(&buf[0..n]).await {
                eprintln!("failed to write to socket; err = {:?}", e);
            }
        }
        Err(e) => {
            eprintln!("failed to read from socket; err = {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let server = Server::new("127.0.0.1:8080").await;

    if let Some(mut server) = server {
        server.listen().await;
    } else {
        error!("Connection not created!");
    }
}