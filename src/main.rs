pub mod server;
pub mod db;

use log::error;
use server::server::Server;

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