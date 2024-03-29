mod arena_linked_list;
mod lru;

pub use lru::LruCache;
pub use arena_linked_list::ArenaLinkedList;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use hyper::http::Uri;
use std::convert::Infallible;
use std::net::SocketAddr;

pub async fn start() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(forward_request))
    });

    println!("Listening on http://{}", addr);

    Server::bind(&addr)
        .http2_only(true) // Add missing import for the `http2` method
        .serve(make_svc)
        .await
        .expect("Failed starting server");
}

async fn forward_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {

    println!("Forwarding request to: {:?}", req.uri());

    let target_uri = Uri::builder()
        .scheme("http")
        .authority("127.0.0.1:3002")
        .path_and_query(req.uri().path_and_query().unwrap().clone())
        .build()
        .expect("Failed to build target URI");

    let mut forwarded_req = Request::builder()
        .method(req.method())
        .uri(target_uri)
        .version(req.version());

    let headers = forwarded_req.headers_mut().expect("Failed to get headers");
    headers.extend(req.headers().iter().map(|(k, v)| (k.clone(), v.clone())));

    let client = Client::builder()
        .http2_only(true)
        .build_http();
    
    let resp = client
        .request(forwarded_req.body(req.into_body()).expect("Failed building request"))
        .await
        .expect("Failed to send request");

    Ok(resp)
}