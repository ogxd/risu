use risu::{self, RisuServer};
use simplelog::*;
use warp::Filter;
use futures::join;

#[tokio::main]
async fn main()
{
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let echo = warp::any().map(|| "Hello, World!");

    let server_fut = warp::serve(echo)
        .run(([127, 0, 0, 1], 3002));

    let risu_fut = RisuServer::start_from_config_file("benches/qps_http/config.yaml");

    join!(server_fut, risu_fut);
}
