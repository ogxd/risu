use std::convert::Infallible;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

async fn handle_request(req: Request<Body>) -> hyper::Result<Response<Body>> {
    //let bytes = hyper::body::aggregate(req.into_body()).await?;
    println!("Received !");
    let bytes = hyper::body::to_bytes(req.into_body()).await?;
    println!("Received payload: {} bytes", bytes.len());
    Ok(Response::new(Body::from("Received")))
}

#[tokio::main]
async fn main() {
    println!("Start !");
    let make_svc = make_service_fn(|_conn| {
        async {
            Ok::<_, Infallible>(service_fn(handle_request))
        }
    });

    let addr = ([127, 0, 0, 1], 3100).into();
    let server = Server::bind(&addr)
        .http2_only(false)
        .http2_initial_stream_window_size(5 * 1024 * 1024) // 5MB
        .http2_initial_connection_window_size(10 * 1024 * 1024) // 10MB
        .serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}