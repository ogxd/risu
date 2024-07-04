use risu::RisuServer;
use simplelog::*;

#[tokio::main]
async fn main()
{
    println!("█▀▀█ █▀▀█ █▀▀ █▀▀█ █░░█ █▀▀ █▀▀ ▀▀█▀▀\n█░░█ █▄▄▀ █▀▀ █░░█ █░░█ █▀▀ ▀▀█ ░░█░░\n█▀▀▀ ▀░▀▀ ▀▀▀ ▀▀▀█ ░▀▀▀ ▀▀▀ ▀▀▀ ░░▀░░");

    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    RisuServer::start_from_config_file("/etc/risu.yml").await;
}
