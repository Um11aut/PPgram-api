#![feature(vec_push_within_capacity)]
#![feature(new_range_api)]

pub mod db;
pub mod server;
pub mod fs;

use db::db::create_tables;
use log::{error, info};
use server::server::Server;

#[tokio::main]
async fn main() {
    create_tables().await;
    env_logger::init();
    let server = Server::new("0.0.0.0:8000", "0.0.0.0:3000").await;

    if let Some(mut server) = server {
        server.poll_events().await;
    }
}
