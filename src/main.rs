#![feature(vec_push_within_capacity)]

pub mod server;
pub mod db;

use log::error;
use server::server::Server;
use db::db::init_dbs;

#[tokio::main]
async fn main() {
    init_dbs().await;

    env_logger::init();
    let server = Server::new("0.0.0.0:8080").await;

    if let Some(mut server) = server {
        server.listen().await;
    } else {
        error!("Connection not created!");
    }
}