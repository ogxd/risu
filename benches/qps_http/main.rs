use std::{convert::Infallible, net::SocketAddr};

use hyper::{service::{service_fn}, Request, Response};
use risu::{self, RisuServer};
use simplelog::*;
use tokio::sync::oneshot;

pub struct TestServer {
    server_handle: tokio::task::JoinHandle<()>,
    shutdown_sender: oneshot::Sender<()>,
}

async fn hello_handler(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
    // let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    // let response_body = format!("hello {}", body_str);
    // Ok(Response::new(Body::from(response_body)))
    panic!();
}

impl TestServer {
    pub fn new_grpc() -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let server_handle = tokio::spawn(async move {
            let addr = SocketAddr::from(([127, 0, 0, 1], 3002));
            let make_svc = make_service_fn(|_conn| async {
                Ok::<_, Infallible>(service_fn(hello_handler))
            });
            let server = Server::bind(&addr).serve(make_svc);
            tokio::select! {
                _ = server => {},
                _ = shutdown_receiver => {
                    // Shutdown signal received
                    println!("Shutting down the server...");
                }
            }
        });
        Self {
            server_handle,
            shutdown_sender,
        }
    }

    pub async fn shutdown(self) {
        self.shutdown_sender.send(()).unwrap();
        self.server_handle.await.unwrap();
    }
}

// grpcurl -plaintext -import-path ./proto -proto hello.proto -d '{"name": "Tonic"}' '127.0.0.1:3001' helloworld.Greeter/SayHello

#[tokio::main]
async fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let server = TestServer::new_grpc();

    RisuServer::start_from_config_file("benches/qps/config.yaml").await;

    server.shutdown().await;
}
