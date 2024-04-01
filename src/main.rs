#[macro_use]
extern crate log;

use simplelog::*;

use risu::RisuServer;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    info!("Starting risu...");

    // RisuServer {
    //     listening_port: 3001,
    //     target_socket_addr: SocketAddr::from(([127, 0, 0, 1], 3002)),
    // }
    RisuServer::start().await;
}
