use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));

    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(forward_request))
    });

    println!("Listening on http://{}", addr);

    Server::bind(&addr)
        .http2_only(false) // Add missing import for the `http2` method
        .serve(make_svc)
        .await
        .expect("server failed");
}

async fn forward_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let client = Client::<HttpConnector<GaiResolver>>::new();
    let resp = client
        .request(
            Request::builder()
                .uri("http://127.0.0.1:3002")
                .body(req.into_body())
                .unwrap(),
        )
        .await
        .expect("failed to forward request");

    Ok(resp)
}