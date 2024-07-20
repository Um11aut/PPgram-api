pub mod server;
use std::{net::SocketAddr, sync::Arc};

use server::message::RequestMessage;
use log::error;
use log::info;
use log::warn;
use log::LevelFilter;
use server::server::Server;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};

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