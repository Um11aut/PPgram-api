#![feature(vec_push_within_capacity)]
#![feature(new_range_api)]
#![feature(addr_parse_ascii)]

pub mod db;
pub mod server;
pub mod fs;

use db::db::create_tables;
use log::error;
use server::server::Server;

const JSON_MESSAGES_PORT: u16 = 3000;
const FILE_MESSAGES_PORT: u16 = 8080;

#[tokio::main]
async fn main() {
    create_tables().await;
    env_logger::init();
    let server = Server::new(JSON_MESSAGES_PORT, FILE_MESSAGES_PORT).await;

    match server {
        Ok(server) => server.poll_events().await,
        Err(err) => error!("Error while creating server: {}", err)
    }
}
