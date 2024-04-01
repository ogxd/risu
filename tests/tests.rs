use std::time::Duration;

use hello_world::greeter_client::GreeterClient;
use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use risu::{self, RisuConfiguration, RisuServer};
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
        println!("Got a request: {:?}", request);

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

    pub fn new_risu() -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let server_handle = tokio::spawn(async move {
            let start_fut = RisuServer::start_from_config_file("tests/config.yaml");
            tokio::select! {
                _ = start_fut => {},
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

#[tokio::test]
async fn grpc() {
    let server = TestServer::new_grpc();
    let risu = TestServer::new_risu();

    // Warmup
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = GreeterClient::connect("http://127.0.0.1:3001").await.unwrap();

    let request = tonic::Request::new(HelloRequest { name: "Tonic".into() });

    let response = client.say_hello(request).await.unwrap();

    server.shutdown().await;
    risu.shutdown().await;

    // Check grpc message content
    assert!(response.get_ref().message == "Hello Tonic!");
}
