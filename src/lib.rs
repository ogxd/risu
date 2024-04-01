#[macro_use]
extern crate log;

mod caches;
mod collections;

pub use caches::*;
pub use collections::*;

use hyper::body::to_bytes;
use hyper::http::Uri;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use std::convert::Infallible;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::lru::ExpirationType;

pub struct RisuServer {
    pub listening_port: u16,
    pub target_socket_addr: SocketAddr,
}

impl RisuServer {
    pub async fn start(&self) {
        let addr = SocketAddr::from(([0, 0, 0, 0], self.listening_port));
        info!("Listening on http://{}", addr);

        let cache = Arc::new(ShardedCache::<u128, Response<Body>>::new(
            8,
            100_000,
            Duration::from_secs(600),
            ExpirationType::Absolute,
        ));

        // let make_svc = make_service_fn(|_conn| async {
        //     Ok::<_, hyper::Error>(service_fn(|_req| handle_request()))
        // });

        // let server = Server::bind(&addr).serve(make_svc);

        let make_svc = make_service_fn(move |_conn| {
            let cache = cache.clone();
            async move { Ok::<_, Infallible>(service_fn(move |req| Self::handle_request(cache.clone(), req))) }
        });

        Server::bind(&addr)
            .http2_only(true) // Add missing import for the `http2` method
            .serve(make_svc)
            .await
            .expect("Failed starting server");
    }

    pub async fn handle_request(
        cache: Arc<ShardedCache<u128, Response<Body>>>, req: Request<Body>,
    ) -> Result<Response<Body>, hyper::Error> {
        debug!("Received request from {:?}", req.uri());

        //let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();

        let closure = |r: Request<Body>| async move {
            let target_uri = Uri::builder()
                .scheme("http")
                .authority("127.0.0.1:3002")
                .path_and_query(r.uri().path_and_query().unwrap().clone())
                .build()
                .expect("Failed to build target URI");

            let mut forwarded_req = Request::builder()
                .method(r.method())
                .uri(target_uri)
                .version(r.version());

            let headers = forwarded_req.headers_mut().expect("Failed to get headers");
            headers.extend(r.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

            let client = Client::builder().http2_only(true).build_http();

            let resp = client
                .request(forwarded_req.body(r.into_body()).expect("Failed building request"))
                .await
                .expect("Failed to send request");

            Ok(resp)
        };

        let result = cache
            .get_or_add_from_item2(
                req,
                |r: &Request<Body>| {
                    // Hash request content
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    r.uri().path().hash(&mut hasher);
                    r.uri().query().hash(&mut hasher);
                    //let bb = r.into_body();
                    //let b = to_bytes(bb);
                    
                    123u128
                },
                closure,
            )
            .await;

        return Ok(Arc::try_unwrap(result.unwrap()).unwrap());
    }
}
