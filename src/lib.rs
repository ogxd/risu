#[macro_use]
extern crate log;

mod executor;
mod buffered_body;
mod caches;
mod collections;
pub mod config;

use buffered_body::BufferedBody;
use executor::TokioExecutor;
pub use caches::*;
pub use collections::*;
pub use config::RisuConfiguration;

use futures::Future;
use gxhash::GxHasher;
use hyper::body::Incoming;
use hyper::http::Uri;
use hyper::server::conn::http2;
use hyper::service::Service;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rand::Rng;
use std::convert::Infallible;
use std::hash::Hash;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: Arc<ShardedCache<u128, Response<BufferedBody>>>,
    //client: Client<HttpConnector>,
}

impl RisuServer {
    pub async fn start_from_config_str(config_str: &str) {
        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(config_str).unwrap();
        RisuServer::start(configuration).await.unwrap();
    }

    pub async fn start_from_config_file(config_file: &str) {
        let contents = std::fs::read_to_string(config_file).expect("Should have been able to read the file");
        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(&contents).unwrap();
        RisuServer::start(configuration).await.unwrap();
    }

    pub async fn start(configuration: RisuConfiguration) -> Result<(), std::io::Error> {
        let server = RisuServer {
            configuration: configuration.clone(),
            cache: Arc::new(ShardedCache::<u128, Response<BufferedBody>>::new(
                configuration.in_memory_shards as usize,
                configuration.cache_resident_size,
                Duration::from_secs(600),
                lru::ExpirationType::Absolute,
            )),
            //client: Client::builder().http2_only(true).build_http(),
        };

        let addr = SocketAddr::from(([0, 0, 0, 0], server.configuration.listening_port));
        info!("Listening on http://{}, http2:{}", addr, configuration.http2);

        // We create a TcpListener and bind it to 127.0.0.1:3000
        let listener = TcpListener::bind(addr).await?;

        // We start a loop to continuously accept incoming connections
        loop {
            let (stream, _) = listener.accept().await?;
            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);
            let server = server.clone();
            tokio::task::spawn(async move {
                if let Err(err) = http2::Builder::new(TokioExecutor).serve_connection(io, server).await {
                    info!("Failed to serve connection: {:?}", err);
                }
            });
        }
    }

    pub async fn call_async(&self, request: Request<Incoming>) -> Result<Response<BufferedBody>, Infallible> {
        let server = self.clone();

        let key_factory = |request: &Request<BufferedBody>| {
            // Hash request content
            let mut hasher = GxHasher::with_seed(123);
            request.uri().path().hash(&mut hasher);
            request.uri().query().hash(&mut hasher);
            request.body().hash(&mut hasher); // Todo: Make this more seamless
            hasher.finish_u128()
        };

        // Round robin target
        let random_number = rand::thread_rng().gen_range(0..server.configuration.target_addresses.len());
        let target_address = server.configuration.target_addresses[random_number].clone(); // Todo: Avoid cloning on every request

        let value_factory = |request: Request<BufferedBody>| async move {
            info!("Cache miss");

            let target_uri = Uri::builder()
                .scheme("http")
                .authority(target_address.clone())
                .path_and_query(request.uri().path_and_query().unwrap().clone())
                .build()
                .expect("Failed to build target URI");

            // Open a TCP connection to the remote host
            // Todo: Connect and handshake only once and reuse the connection
            let stream = TcpStream::connect(target_address).await.expect("Connection failed");

            info!("Connected");

            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);

            // Create the Hyper client
            let (mut sender, conn) = hyper::client::conn::http2::handshake(TokioExecutor, io)
                .await
                .expect("Handshake failed");

            info!("Handshake completed");

            // Spawn a task to poll the connection, driving the HTTP state
            // Todo: Is this necessary?
            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    error!("Connection failed: {:?}", err);
                }
            });

            // Copy path and query
            let mut forwarded_req = Request::builder()
                .method(request.method())
                .uri(target_uri)
                .version(request.version());

            // Copy headers
            let headers = forwarded_req.headers_mut().expect("Failed to get headers");
            headers.extend(request.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

            let body = request.into_body();

            info!("Buffering request...");

            // Copy body
            let forwarded_req = forwarded_req.body(body).expect("Failed building request");

            info!("Forwarding request");

            // Await the response...
            let res: Response<Incoming> = sender
                .send_request(forwarded_req)
                .await
                .expect("Failed to send request");

            // Buffer response body so that we can cache it and return it
            let (parts, body) = res.into_parts();
            let collected = BufferedBody::collect_buffered(body).await.unwrap();

            info!("Received response from target with status: {:?}", parts.status);

            Ok(Response::from_parts(parts, collected))
        };

        let (parts, body) = request.into_parts();
        let buffered_body = BufferedBody::collect_buffered(body).await.unwrap();
        let request = Request::from_parts(parts, buffered_body);

        let result: Result<Arc<Response<BufferedBody>>, ()> = server
            .cache
            .get_or_add_from_item2(request, key_factory, value_factory)
            .await;

        match result {
            Ok(response) => {
                let response = response.as_ref();
                let response: Response<BufferedBody> = response.clone();
                info!("Received response from target with status: {:?}", response);
                return Ok(response);
            }
            Err(_) => panic!(),
        }
    }
}

impl Service<Request<Incoming>> for RisuServer {
    type Response = Response<BufferedBody>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>; // Wrong type

    fn call(&self, request: Request<Incoming>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.call_async(request).await })
    }
}