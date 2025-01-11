#![feature(vec_push_within_capacity)]
#![feature(new_range_api)]
#![feature(addr_parse_ascii)]

pub mod db;
pub mod fs;
pub mod server;

use db::init::create_tables;
use log::error;
use server::server::Server;

const JSON_MESSAGES_PORT: u16 = 3000;
const FILE_MESSAGES_PORT: u16 = 8080;

#[cfg(debug_assertions)]
fn init_logging() {
    env_logger::init();
}

#[cfg(not(debug_assertions))]
fn init_logging() {
    use std::io::Write;
    use chrono::Local;
    use log::LevelFilter;

    let now = Local::now().format("%Y-%m-%d-%H-%M");
    let target =
        Box::new(std::fs::File::create(format!("/tmp/{}.log", now)).expect("Can't create file"));

    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} [{}] - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(target))
        .filter(None, LevelFilter::max())
        .init();
}

#[tokio::main]
async fn main() {
    init_logging();

    create_tables().await;
    let server = Server::new(JSON_MESSAGES_PORT, FILE_MESSAGES_PORT).await;

    match server {
        Ok(server) => server.poll_events().await,
        Err(err) => error!("Error while creating server: {}", err),
    }
}
