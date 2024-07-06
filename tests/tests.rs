include!("../proto/helloworld.rs");

use std::{clone, time::Duration};

use calculator_client::CalculatorClient;
use calculator_server::{Calculator, CalculatorServer};
use risu::{self, RisuConfiguration, RisuServer};
use tokio::sync::oneshot;
use tonic::{metadata::MetadataMap, metadata::MetadataValue, transport::Server, Extensions, Request, Response, Status};
use warp::Filter;

#[derive(Debug, Default)]
pub struct MyCalculator {}

#[tonic::async_trait]
impl Calculator for MyCalculator
{
    async fn sum(&self, request: Request<CalculationRequest>) -> Result<Response<CalculationResult>, Status>
    {
        println!("Got a request: {:?}", request);

        let request = request.into_inner();

        let reply = CalculationResult {
            result: request.a + request.b,
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
            .add_service(CalculatorServer::new(MyCalculator::default()))
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

    let mut client = CalculatorClient::connect(format!("http://127.0.0.1:{}", risu_port)).await.unwrap();

    let mut metadata = MetadataMap::new();
    metadata.insert("x-target-host", format!("127.0.0.1:{}", target_port).parse().unwrap());

    let request = tonic::Request::from_parts(
        metadata,
        Extensions::default(),
        CalculationRequest { a: 1, b: 1 });
    
    let response = client.sum(request).await.unwrap();

    // Check grpc message content
    assert!(response.get_ref().result == 2);

    server.shutdown().await;
    risu.shutdown().await;
}

#[tokio::test]
async fn grpc_many()
{
    // CombinedLogger::init(vec![TermLogger::new(
    //     LevelFilter::Debug,
    //     Config::default(),
    //     TerminalMode::Mixed,
    //     ColorChoice::Auto,
    // )])
    // .unwrap();

    let risu_port = 3008;
 
    let config = RisuConfiguration {
        listening_port: risu_port,
        ..Default::default()
    };

    let server1 = TestServer::new_grpc(format!("127.0.0.1:{}", 3410));
    let server2 = TestServer::new_grpc(format!("127.0.0.1:{}", 3411));
    let server3 = TestServer::new_grpc(format!("127.0.0.1:{}", 3412));
    let risu = TestServer::new_risu(config);

    // Let servers start and warmup
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client = CalculatorClient::connect(format!("http://127.0.0.1:{}", risu_port)).await.unwrap();

    for _ in 0..100
    {
        // Get random number between 0 and 2
        let target_port = 3410 + (rand::random::<u32>() % 3);
        let mut metadata = MetadataMap::new();
        metadata.insert("x-target-host", format!("127.0.0.1:{}", target_port).parse().unwrap());

        // Get random string
        let a = rand::random::<u8>() as i32;
        let b = rand::random::<u8>() as i32;

        let request = tonic::Request::from_parts(
            metadata,
            Extensions::default(),
            CalculationRequest { a: a, b: b});
        
        let response = client.sum(request).await.unwrap();

        // Check grpc message content
        assert!(response.get_ref().result == a + b);
    }

    server1.shutdown().await;
    server2.shutdown().await;
    server3.shutdown().await;
    risu.shutdown().await;
}