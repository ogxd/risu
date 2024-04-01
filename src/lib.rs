#[macro_use]
extern crate log;

mod caches;
mod collections;

pub use caches::*;
pub use collections::*;

use gxhash::GxHasher;
use hyper::body::Bytes;
use hyper::http::Uri;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use serde::Deserialize;
use std::convert::Infallible;
use std::hash::Hash;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::lru::ExpirationType;

#[derive(Debug, Deserialize)]
pub struct RisuConfiguration {
    pub listening_port: u16,
    pub target_address: String,
}

pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: ShardedCache<u128, Response<Bytes>>,
}

impl RisuServer {
    pub async fn start(configuration: RisuConfiguration) {
        let server = Arc::new(RisuServer {
            configuration: configuration,
            cache: ShardedCache::<u128, Response<Bytes>>::new(
                8,
                100_000,
                Duration::from_secs(600),
                ExpirationType::Absolute,
            ),
        });

        let addr = SocketAddr::from(([0, 0, 0, 0], server.configuration.listening_port));
        info!("Listening on http://{}", addr);

        let make_svc = make_service_fn(move |_conn| {
            let server = server.clone();
            async move { Ok::<_, Infallible>(service_fn(move |req| RisuServer::handle_request(server.clone(), req))) }
        });

        Server::bind(&addr)
            .http2_only(true) // Add missing import for the `http2` method
            .serve(make_svc)
            .await
            .expect("Failed starting server");
    }

    pub async fn handle_request(
        s: Arc<Self>,
        //cache: Arc<ShardedCache<u128, Response<Bytes>>>,
        request: Request<Body>,
    ) -> Result<Response<Body>, hyper::Error> {
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

        let target_address = s.configuration.target_address.clone(); // Todo: Avoid cloning on every request

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

            let client = Client::builder().http2_only(true).build_http();

            let resp = client.request(forwarded_req).await.expect("Failed to send request");

            // Buffer response body so that we can cache it and return it
            let (parts, body) = resp.into_parts();
            let body_bytes: Bytes = hyper::body::to_bytes(body).await.unwrap();
            let response_buffered = Response::from_parts(parts, body_bytes);

            Ok(response_buffered)
        };

        let result: Result<Arc<Response<Bytes>>, ()> = s
            .cache
            .get_or_add_from_item2(request_buffered, key_factory, value_factory)
            .await;

        match result {
            Ok(response) => {
                let response: Response<Body> = Arc::try_unwrap(response).unwrap().map(|bytes| Body::from(bytes));
                return Ok(response);
            }
            Err(_) => return Ok(Response::builder().status(500).body(Body::empty()).unwrap()),
        }
    }
}
