use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use risu::{self, RisuServer};
use simplelog::*;
use tokio::sync::oneshot;
use tonic::{transport::Server, Request, Response, Status};

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(&self, request: Request<HelloRequest>) -> Result<Response<HelloReply>, Status> {
        let reply = hello_world::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(reply))
    }
}

pub struct TestServer {
    server_handle: tokio::task::JoinHandle<()>,
    shutdown_sender: oneshot::Sender<()>,
}

impl TestServer {
    pub fn new_grpc() -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let server_handle = tokio::spawn(async move {
            let server = Server::builder()
                .add_service(GreeterServer::new(MyGreeter::default()))
                .serve("127.0.0.1:3002".parse().unwrap());
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
        LevelFilter::Warn,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let server = TestServer::new_grpc();

    RisuServer::start_from_config_file("benches/qps/config.yaml").await;

    server.shutdown().await;
}
