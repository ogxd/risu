#[macro_use]
extern crate log;

mod caches;
mod collections;
pub mod config;

pub use caches::*;
pub use collections::*;
pub use config::RisuConfiguration;

use gxhash::GxHasher;
use hyper::body::{Bytes, HttpBody};
use hyper::client::HttpConnector;
use hyper::header::HeaderValue;
use hyper::http::Uri;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, HeaderMap, Request, Response, Server, StatusCode, Version};
use rand::Rng;
use std::convert::Infallible;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: ShardedCache<u128, BufferedResponse>,
    client: Client<HttpConnector>,
}

#[derive(Debug, Clone)]
pub struct BufferedResponse {
    /// The response's status
    pub status: StatusCode,
    /// The response's version
    version: Version,
    /// The response's headers
    headers: HeaderMap<HeaderValue>,
    /// The response's body
    body: hyper::body::Bytes,
}

impl BufferedResponse {
    pub async fn from(response: Response<Body>) -> BufferedResponse {
        let (parts, body) = response.into_parts();
        BufferedResponse {
            status: parts.status,
            version: parts.version,
            headers: parts.headers,
            body: hyper::body::to_bytes(body).await.unwrap(),
        }
    }
    pub fn to(&self) -> Response<Body> {
        let mut builder = Response::builder().status(self.status).version(self.version);

        let headers = builder.headers_mut().expect("Failed to get headers");
        headers.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));

        builder.body(Body::from(self.body.clone())).unwrap()
    }
}

impl RisuServer {
    pub async fn start_from_config_str(config_str: &str) {
        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(config_str).unwrap();
        RisuServer::start(configuration).await;
    }

    pub async fn start_from_config_file(config_file: &str) {
        let contents = std::fs::read_to_string(config_file).expect("Should have been able to read the file");
        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(&contents).unwrap();
        RisuServer::start(configuration).await;
    }

    pub async fn start(configuration: RisuConfiguration) {
        let server = Arc::new(RisuServer {
            configuration: configuration.clone(),
            cache: ShardedCache::<u128, BufferedResponse>::new(
                configuration.in_memory_shards as usize,
                configuration.cache_resident_size,
                Duration::from_secs(600),
                lru::ExpirationType::Absolute,
            ),
            client: Client::builder().http2_only(true).build_http(),
        });

        let addr = SocketAddr::from(([0, 0, 0, 0], server.configuration.listening_port));
        info!("Listening on http://{}, http2:{}", addr, configuration.http2);

        let make_svc = make_service_fn(move |_conn| {
            let server = server.clone();
            //async move { Ok::<_, Infallible>(service_fn(move |req| RisuServer::handle_request(server.clone(), req))) }
            async move { Ok::<_, Infallible>(service_fn(move |req| RisuServer::handle_request_no_caching(server.clone(), req))) }
        });

        Server::bind(&addr)
            .http2_only(configuration.http2)
            .serve(make_svc)
            .await
            .expect("Failed starting server");
    }

    pub async fn handle_request(server: Arc<Self>, request: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        debug!("Received request from {:?}", request.uri());

        // Buffer request body so that we can hash it and forward it
        let (parts, body) = request.into_parts();
        let body_bytes: Bytes = hyper::body::to_bytes(body).await.unwrap();
        let request_buffered = Request::from_parts(parts, body_bytes);

        let key_factory = |request: &Request<Bytes>| {
            // Hash request content
            let mut hasher = GxHasher::with_seed(123);
            request.uri().path().hash(&mut hasher);
            request.uri().query().hash(&mut hasher);
            request.body().hash(&mut hasher);
            hasher.finish_u128()
        };

        // Round robin target
        let random_number = rand::thread_rng().gen_range(0..server.configuration.target_addresses.len());
        let target_address = server.configuration.target_addresses[random_number].clone(); // Todo: Avoid cloning on every request

        let client = &server.clone().client;

        let value_factory = |request: Request<Bytes>| async move {
            debug!("Cache miss");

            let target_uri = Uri::builder()
                .scheme("http")
                .authority(target_address)
                .path_and_query(request.uri().path_and_query().unwrap().clone())
                .build()
                .expect("Failed to build target URI");

            // Copy path and query
            let mut forwarded_req = Request::builder()
                .method(request.method())
                .uri(target_uri)
                .version(request.version());

            // Copy headers
            let headers = forwarded_req.headers_mut().expect("Failed to get headers");
            headers.extend(request.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

            // Copy body
            let forwarded_req: Request<Bytes> = forwarded_req
                .body(request.into_body())
                .expect("Failed building request");

            let forwarded_req: Request<Body> = forwarded_req.map(|bytes| Body::from(bytes));

            debug!("Forwarding request");

            let resp = client.request(forwarded_req).await.expect("Failed to send request");

            // Buffer response body so that we can cache it and return it
            let response_buffered = BufferedResponse::from(resp).await;

            debug!("Received response from target with status: {:?}", response_buffered);

            Ok(response_buffered)
        };

        let result: Result<Arc<BufferedResponse>, ()> = server
            .cache
            .get_or_add_from_item2(request_buffered, key_factory, value_factory)
            .await;

        match result {
            Ok(response) => {
                let response = response.to();
                debug!("Received response from target with status: {:?}", response);
                return Ok(response);
            }
            Err(_) => return Ok(Response::builder().status(500).body(Body::empty()).unwrap()),
        }
    }

    pub async fn handle_request_no_caching(server: Arc<Self>, request: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        debug!("Received request from {:?}", request.uri());

        let random_number = rand::thread_rng().gen_range(0..server.configuration.target_addresses.len());
        let target_address = server.configuration.target_addresses[random_number].clone(); // Todo: Avoid cloning on every request

        let target_uri = Uri::builder()
            .scheme("http")
            .authority(target_address)
            .path_and_query(request.uri().path_and_query().unwrap().clone())
            .build()
            .expect("Failed to build target URI");

        // Copy path and query
        let mut forwarded_req = Request::builder()
            .method(request.method())
            .uri(target_uri)
            .version(request.version());

        // Copy headers
        let headers = forwarded_req.headers_mut().expect("Failed to get headers");
        headers.extend(request.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

        // Copy body
        let forwarded_req: Request<Body> = forwarded_req
            .body(request.into_body())
            .expect("Failed building request");

        let forwarded_req: Request<Body> = forwarded_req.map(|bytes| Body::from(bytes));

        let client = Client::builder().http2_only(true).build_http();

        debug!("Forwarding request");

        let resp = client.request(forwarded_req).await.expect("Failed to send request");

        debug!("Received response from target with status: {:?}", resp.status());

        // Getting a "server closed the stream without sending trailers" error from client with this
        //let resp = BufferedResponse::from(resp).await.to();

        // Getting a "server closed the stream without sending trailers" error from client with this
        // let (parts, body) = resp.into_parts();
        // let body_bytes = hyper::body::to_bytes(body).await.unwrap();
        // let resp = Response::from_parts(parts, Body::from(body_bytes));

        return Ok(resp);
    }
}
