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
    let json1 = r#"
    {
        "method": "sendMedia",
        "message_id": 123,
        "to": 111111,
        "date": 3819273812,
        "has_reply": true,
        "reply_to": 12312312,
        "content": {
            "media": [
                {
                    "file_name": "test.png",
                    "format": "png",
                    "data": "nvmxcnfiowerldjlksdjflksdjflksdfklsd"
                },
                {
                    "file_name": "huy.mp4",
                    "format": "mp4",
                    "data": "nvmxcnfiowerldjlksdjflksdjflksdfklsd"
                }
            ],
            "has_caption": true,
            "caption": "Pepuk gay"
        }
    }"#;

    let json2 = r#"
    {"method":"sendMessage","message_id":123,"to":111111,"date":3819273812,"has_reply":true,"reply_to":12312312,"content":{"text":"Pepukpidor"}}
    "#;

    let message1: RequestMessage = serde_json::from_str(json1).unwrap();
    let message2: RequestMessage = serde_json::from_str(json2).unwrap();

    println!("{:?}", message1);
    println!("{:?}", message2);

    let server = Server::new("127.0.0.1:8080").await;

    if let Some(mut server) = server {
        server.listen().await;
    } else {
        error!("Connection not created!");
    }
}