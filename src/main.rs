#![feature(vec_push_within_capacity)]
#![feature(new_range_api)]
#![feature(addr_parse_ascii)]

pub mod db;
pub mod server;
pub mod fs;

use db::db::create_tables;
use log::error;
use server::server::Server;

#[tokio::main]
async fn main() {
    create_tables().await;
    env_logger::init();
    // 1: QUIC Port
    // 2: TCP Port
    let server = Server::new(3000, 8080).await;

    match server {
        Ok(server) => server.poll_events().await,
        Err(err) => error!("Error while creating server: {}", err)
    }
}
