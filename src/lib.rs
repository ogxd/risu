#[macro_use]
extern crate log;

mod buffered_body;
mod caches;
mod collections;
pub mod config;
mod executor;

use buffered_body::BufferedBody;
pub use caches::*;
pub use collections::*;
pub use config::RisuConfiguration;
use executor::TokioExecutor;

use futures::future::join;
use gxhash::GxHasher;
use hyper::body::Incoming;
use hyper::http::Uri;
use hyper::server::conn::{http1, http2};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use rand::Rng;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

use prometheus::{Counter, Encoder, Histogram, HistogramOpts, Opts, Registry, TextEncoder};

pub struct Metrics {
    response_time: Histogram,
    cache_calls: Counter,
    // cache_hits: Counter,
    cache_misses: Counter,
    // cache_evictions: Counter,
    // cache_resident_size: Counter,
    // cache_probatory_size: Counter,
    registry: Registry,
}

impl Metrics {
    pub fn new() -> Metrics {
        let metrics = Metrics {
            response_time: Histogram::with_opts(HistogramOpts::new("response_time", "Response time").buckets(vec![
                0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1., 2., 5., 10., 20., 50., 100., 200., 500., 1000., 2000., 5000.,
                10000.,
            ]))
            .unwrap(),
            cache_calls: Counter::with_opts(Opts::new("cache_calls", "Number of cache calls")).unwrap(),
            cache_misses: Counter::with_opts(Opts::new("cache_misses", "Number of cache misses")).unwrap(),
            registry: Registry::new(),
        };
        metrics
            .registry
            .register(Box::new(metrics.response_time.clone()))
            .unwrap();
        metrics
            .registry
            .register(Box::new(metrics.cache_calls.clone()))
            .unwrap();
        metrics
            .registry
            .register(Box::new(metrics.cache_misses.clone()))
            .unwrap();
        metrics
    }
}

pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: ShardedCache<u128, Response<BufferedBody>>,
    metrics: Metrics,
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
        let server = Arc::new(RisuServer {
            configuration: configuration.clone(),
            cache: ShardedCache::<u128, Response<BufferedBody>>::new(
                configuration.in_memory_shards as usize,
                configuration.cache_resident_size,
                Duration::from_secs(600),
                lru::ExpirationType::Absolute,
            ),
            metrics: Metrics::new(),
            //client: Client::builder().http2_only(true).build_http(),
        });

        let service = async {
            let service_address = SocketAddr::from(([0, 0, 0, 0], server.configuration.listening_port));
            info!(
                "Service listening on http://{}, http2:{}",
                service_address, configuration.http2
            );

            let listener = TcpListener::bind(service_address).await.unwrap();

            // We start a loop to continuously accept incoming connections
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                // Use an adapter to access something implementing `tokio::io` traits as if they implement
                // `hyper::rt` IO traits.
                let io = TokioIo::new(stream);
                let server = server.clone();
                tokio::task::spawn(async move {
                    //let server = server.clone();
                    if let Err(err) = http2::Builder::new(TokioExecutor)
                        .serve_connection(io, service_fn(move |req| RisuServer::call_async(server.clone(), req)))
                        .await
                    {
                        warn!("Error serving connection: {:?}", err);
                    }
                });
            }
        };

        let prometheus = async {
            let prom_address: SocketAddr = ([127, 0, 0, 1], server.configuration.prometheus_port).into();
            info!("Prometheus listening on http://{}", prom_address);
            let listener = TcpListener::bind(prom_address).await.unwrap();

            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = TokioIo::new(stream);
                let server = server.clone();
                tokio::task::spawn(async move {
                    //let server = server.clone();
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(io, service_fn(move |req| RisuServer::prometheus(server.clone(), req)))
                        .await
                    {
                        warn!("Error serving prom connection: {:?}", err);
                    }
                });
            }
        };

        _ = join(service, prometheus).await;

        Ok(())
    }

    pub async fn prometheus(
        server: Arc<RisuServer>, request: Request<hyper::body::Incoming>,
    ) -> Result<Response<BufferedBody>, hyper::Error> {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = server.metrics.registry.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        Ok(Response::new(BufferedBody::from_bytes(&buffer)))
    }

    pub async fn call_async(
        service: Arc<RisuServer>, request: Request<Incoming>,
    ) -> Result<Response<BufferedBody>, hyper::Error> {
        let timestamp = std::time::Instant::now();
        service.metrics.cache_calls.inc();

        let key_factory = |request: &Request<BufferedBody>| {
            // Hash request content
            let mut hasher = GxHasher::with_seed(123);
            request.uri().path().hash(&mut hasher);
            request.uri().query().hash(&mut hasher);
            request.body().hash(&mut hasher); // Todo: Make this more seamless
            hasher.finish_u128()
        };

        // Round robin target
        let random_number = rand::thread_rng().gen_range(0..service.configuration.target_addresses.len());
        let target_address = &service.configuration.target_addresses[random_number];

        let value_factory = |request: Request<BufferedBody>| async {
            info!("Cache miss");
            service.metrics.cache_misses.inc();

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

        let result: Result<Arc<Response<BufferedBody>>, hyper::Error> = service
            .cache
            .get_or_add_from_item2(request, key_factory, value_factory)
            .await;

        let response = match result {
            Ok(response) => {
                let response = response.as_ref();
                let response: Response<BufferedBody> = response.clone();
                info!("Received response from target with status: {:?}", response);
                Ok(response)
            }
            Err(e) => Err(e),
        };

        let elapsed = timestamp.elapsed();
        service.metrics.response_time.observe(elapsed.as_millis() as f64);

        response
    }
}
