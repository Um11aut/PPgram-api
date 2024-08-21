#![feature(vec_push_within_capacity)]
#![feature(new_range_api)]

pub mod db;
pub mod server;

use db::db::init_dbs;
use log::error;
use server::server::Server;

#[tokio::main]
async fn main() {
    init_dbs().await;
    
    env_logger::init();
    let server = Server::new("0.0.0.0:8080").await;

    if let Some(mut server) = server {
        server.poll_events().await;
    } else {
        error!("Server not created!");
    }
}
