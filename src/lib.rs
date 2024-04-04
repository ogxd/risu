#[macro_use]
extern crate log;

mod caches;
mod collections;
pub mod config;

use bytes::BufMut;
pub use caches::*;
pub use collections::*;
pub use config::RisuConfiguration;

use futures::Future;
use gxhash::GxHasher;
use http_body_util::{BodyExt, Collected, Full};
use hyper::body::{Body, Bytes, Frame, Incoming};
use hyper::rt::Executor;
use hyper::server::conn::{http1, http2};
use hyper::header::HeaderValue;
use hyper::http::Uri;
use hyper::service::service_fn;
use hyper::{Error, HeaderMap, Request, Response, StatusCode, Version};
use hyper_util::rt::TokioIo;
use rand::Rng;
use tokio::net::{TcpListener, TcpStream};
use std::convert::Infallible;
use std::hash::Hash;
use std::io::Read;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use http_body_util::Empty;

use pin_project_lite::pin_project;

pin_project! {
    /// Future that resolves into a [`Collected`].
    ///
    /// [`Collected`]: crate::Collected
    pub struct BufferBody
    {
        pub(crate) collected: Option<BufferedBody>,
        #[pin]
        pub(crate) body: Full<Bytes>,
    }
}

impl Future for BufferBody {
    type Output = Result<BufferedBody, std::io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let mut me = self.project();

        loop {
            let frame = futures_core::ready!(me.body.as_mut().poll_frame(cx));

            let frame = if let Some(frame) = frame {
                frame?
            } else {
                return Poll::Ready(Ok(me.collected.take().expect("polled after complete")));
            };

            me.collected.as_mut().unwrap().push_frame(frame);
        }
    }
}

pub trait BufferedBodyExt: Body {

    /// Turn this body into [`Collected`] body which will collect all the DATA frames
    /// and trailers.
    fn collect(self) -> combinators::Collect<Self>
    where
        Self: Sized,
    {
        combinators::Collect {
            body: self,
            collected: Some(crate::Collected::default()),
        }
    }
}

#[derive(Debug)]
pub struct BufferedBody {
    bufs: Bytes,
    trailers: Option<HeaderMap>,
}

impl BufferedBody {
    /// If there is a trailers frame buffered, returns a reference to it.
    ///
    /// Returns `None` if the body contained no trailers.
    pub fn trailers(&self) -> Option<&HeaderMap> {
        self.trailers.as_ref()
    }

    // Convert this body into a [`Bytes`].
    // pub fn to_bytes(mut self) -> Bytes {
    //     self.bufs.copy_to_bytes(self.bufs.remaining())
    // }

    // pub(crate) fn push_frame(&mut self, frame: Frame<Bytes>) {
    //     let frame = match frame.into_data() {
    //         Ok(data) => {
    //             // Only push this frame if it has some data in it, to avoid crashing on
    //             // `BufList::push`.
    //             if data.has_remaining() {
    //                 self.bufs.push(data);
    //             }
    //             return;
    //         }
    //         Err(frame) => frame,
    //     };

    //     if let Ok(trailers) = frame.into_trailers() {
    //         if let Some(current) = &mut self.trailers {
    //             current.extend(trailers);
    //         } else {
    //             self.trailers = Some(trailers);
    //         }
    //     };
    // }
}

impl Body for BufferedBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(mut self: Pin<&mut Self>, _: &mut Context<'_>)
        -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = if self.bufs.len() > 0 {
            Frame::data(self.bufs.to_owned())
        } else if let Some(trailers) = self.trailers.take() {
            Frame::trailers(trailers)
        } else {
            return Poll::Ready(None);
        };

        Poll::Ready(Some(Ok(frame)))
    }
}

pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: ShardedCache<u128, Response<BufferedBody>>,
    //client: Client<HttpConnector>,
}

// #[derive(Debug, Clone)]
// pub struct BufferedResponse {
//     /// The response's status
//     pub status: StatusCode,
//     /// The response's version
//     version: Version,
//     /// The response's headers
//     headers: HeaderMap<HeaderValue>,
//     /// The response's body
//     body: Bytes,
// }

/*
impl BufferedResponse {
    pub async fn from(response: Response<Body>) -> BufferedResponse {
        
        let (mut parts, body) = response.into_parts();
        let collected: Collected<Bytes> = body.collect().await.unwrap();
        let trailers = collected.trailers().unwrap();
        for (k, v) in trailers.iter() {
            parts.headers.insert(k, v.clone());
        }

        BufferedResponse {
            status: parts.status,
            version: parts.version,
            headers: parts.headers,
            body: collected.to_bytes(),
        }
    }

    // pub async fn forward(request: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    //     let forwarded_request: Request<Body> = ...;
    
    //     let client = Client::builder().http2_only(true).build_http();
    //     let response = client.request(forwarded_request).await.unwrap();
    
    //     let (parts, body) = response.into_parts();
    //     let collected: Collected<Bytes> = body.collect().await.unwrap();
    //     let response = Response::from_parts(parts, Body::from(collected)); // Error
    
    //     Ok(response)
    // }

    pub fn to(&self) -> Response<Body> {
        let mut builder = Response::builder().status(self.status).version(self.version);

        let headers = builder.headers_mut().expect("Failed to get headers");
        headers.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));

        // let body = Bytes::from(self.body.clone());

        // let stream = futures::stream::once(futures::future::ready(Ok::<_, std::io::Error>(body)));
        // let body = Body::wrap_stream(stream);
        //Body::wrap_stream(stream)

        builder.body(Body::from(self.body.clone())).unwrap()
    }
}
*/

#[derive(Clone)]
struct TokioExecutor;

impl<F> Executor<F> for TokioExecutor
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, future: F) {
        tokio::spawn(future);
    }
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
            //client: Client::builder().http2_only(true).build_http(),
        });

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

            // Spawn a tokio task to serve multiple connections concurrently
            tokio::task::spawn(async move {
                let server = server.clone();
                // Finally, we bind the incoming connection to our `hello` service
                if let Err(err) = http2::Builder::new(TokioExecutor)
                    // `service_fn` converts our function in a `Service`
                    .serve_connection(io, service_fn(move |req| RisuServer::hello(server.clone(), req)))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }

    /*
    pub async fn handle_request(server: Arc<Self>, request: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        debug!("Received request from {:?}", request.uri());

        // Buffer request body so that we can hash it and forward it
        let (parts, body) = request.into_parts();
        debug!("Received headers: {:?}", parts);
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
     */

    async fn hello(server: Arc<Self>, request: Request<hyper::body::Incoming>) -> Result<Response<BufferedBody>, Infallible> {

        let key_factory = |request: &Request<BufferedBody>| {
            // Hash request content
            let mut hasher = GxHasher::with_seed(123);
            request.uri().path().hash(&mut hasher);
            request.uri().query().hash(&mut hasher);
            //let k: &Collected<Bytes> = request.body();
            //Full::new(k).hash(&mut hasher);
            //let k = request.clone();
            //k.to_bytes().hash(&mut hasher);
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
            let stream = TcpStream::connect(target_address).await.expect("Connection failed");
    
            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);
    
            // Create the Hyper client
            let (mut sender, conn) = hyper::client::conn::http2::handshake(TokioExecutor, io).await.expect("Handshake failed");
    
            // Spawn a task to poll the connection, driving the HTTP state
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
    
            // Copy body
            let forwarded_req = forwarded_req
                .body(body.collect().await.unwrap())
                .expect("Failed building request");

            debug!("Forwarding request");

            // Await the response...
            let res: Response<Incoming> = sender.send_request(forwarded_req).await.expect("Failed to send request");

            // Buffer response body so that we can cache it and return it
            let (parts, body) = res.into_parts();
            let collected = body.collect().await.unwrap();

            debug!("Received response from target with status: {:?}", parts.status);

            Ok(Response::from_parts(parts, collected))
        };

        let (parts, body) = request.into_parts();
        let collected = body.collect().await.unwrap();
        let request = Request::from_parts(parts, collected);

        let result: Result<Arc<Response<BufferedBody>>, ()> = server
            .cache
            .get_or_add_from_item2(request, key_factory, value_factory)
            .await;

        warn!("Hello, World!");

        //info!("Response status: {}", res.status());

        Ok(Arc::try_unwrap(result.unwrap()).unwrap())
    }
}
