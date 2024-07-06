include!("../proto/helloworld.rs");

use std::{clone, time::Duration};

use greeter_client::GreeterClient;
use greeter_server::{Greeter, GreeterServer};
use risu::{self, RisuConfiguration, RisuServer};
use tokio::sync::oneshot;
use tonic::{metadata::MetadataMap, metadata::MetadataValue, transport::Server, Extensions, Request, Response, Status};
use warp::Filter;

#[derive(Debug, Default)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter
{
    async fn say_hello(&self, request: Request<HelloRequest>) -> Result<Response<HelloReply>, Status>
    {
        println!("Got a request: {:?}", request);

        let reply = HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(reply))
    }
}

pub struct TestServer
{
    server_handle: tokio::task::JoinHandle<()>,
    shutdown_sender: oneshot::Sender<()>,
}

impl TestServer
{
    pub fn new_grpc(address: String) -> Self
    {
        Self::start(Server::builder()
            .add_service(GreeterServer::new(MyGreeter::default()))
            .serve(address.parse().unwrap()))
    }

    pub fn new_risu(config: RisuConfiguration) -> Self
    {
        Self::start(RisuServer::start(config))
    }

    fn start<F>(fut: F) -> Self
        where F : core::future::Future + Send + 'static
    {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let server_handle = tokio::spawn(async move {
            tokio::select! {
                _ = fut => {},
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

    pub async fn shutdown(self)
    {
        self.shutdown_sender.send(()).unwrap();
        self.server_handle.await.unwrap();
    }
}

use simplelog::*;

#[tokio::test]
async fn grpc()
{
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let risu_port = 3001;
    let target_port = 3002;
 
    let config = RisuConfiguration {
        listening_port: risu_port,
        ..Default::default()
    };

    let server = TestServer::new_grpc(format!("127.0.0.1:{}", target_port));
    let risu = TestServer::new_risu(config);

    // Let servers start and warmup
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = GreeterClient::connect(format!("http://127.0.0.1:{}", risu_port)).await.unwrap();

    let mut metadata = MetadataMap::new();
    metadata.insert("x-target-host", format!("127.0.0.1:{}", target_port).parse().unwrap());

    let request = tonic::Request::from_parts(
        metadata,
        Extensions::default(),
        HelloRequest { name: "Cacheus".into() });
    
    let response = client.say_hello(request).await.unwrap();

    server.shutdown().await;
    risu.shutdown().await;

    // Check grpc message content
    assert!(response.get_ref().message == "Hello Cacheus!");
}