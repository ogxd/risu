#[macro_use]
extern crate log;

use simplelog::*;

use risu::RisuServer;
use std::fs;
use risu::RisuConfiguration;

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

    // read the file.
    let contents = fs::read_to_string("config.yaml")
        .expect("Should have been able to read the file");

    // don't unwrap like this in the real world! Errors will result in panic!
    let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(&contents).unwrap();

    RisuServer::start(configuration).await;
}
