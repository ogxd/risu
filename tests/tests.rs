use std::time::Duration;

use risu;
use tokio::sync::oneshot;
use tonic::{transport::Server, Request, Response, Status};
use hello_world::greeter_server::{Greeter, GreeterServer};
use hello_world::{HelloReply, HelloRequest};
use hello_world::greeter_client::GreeterClient;

pub mod hello_world {
    tonic::include_proto!("helloworld");
}

#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        println!("Got a request: {:?}", request);

        let reply = hello_world::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(reply))
    }
}

#[tokio::test]
async fn grpc() {

    // Start the server
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

    // Start Risu
    tokio::spawn(async move {
        risu::start().await;
    });

    // Warmup
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = GreeterClient::connect("http://127.0.0.1:3001").await.unwrap();

    // Create a new HelloRequest message
    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });

    // Send the request to the server
    let response = client.say_hello(request).await.unwrap();

    // To stop the service later, send the shutdown signal
    shutdown_sender.send(()).unwrap();

    // Wait for the server to finish shutting down
    server_handle.await.unwrap();

    // Check grpc message content
    assert!(response.get_ref().message == "Hello Tonic!");
}