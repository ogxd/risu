#[macro_use]
extern crate log;

mod caches;
mod collections;
pub mod config;

use bytes::{Buf, BufMut, BytesMut};
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
use hyper::service::{service_fn, Service};
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
    pub struct BufferBody<T>
    where
        T: Body,
        T: ?Sized,
    {
        pub(crate) collected: Option<BufferedBody>,
        #[pin]
        pub(crate) body: T,
    }
}

impl<T: Body + ?Sized> Future for BufferBody<T> {
    type Output = Result<BufferedBody, T::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let mut me = self.project();

        loop {
            info!("Polling...");

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

pub fn collect_buffered<T>(body: T) -> BufferBody<T>
where
    T: Body,
    T: Sized,
{
    BufferBody {
        body: body,
        collected: Some(BufferedBody::default()),
    }
}

#[derive(Debug, Default, Clone)]
pub struct BufferedBody {
    bufs: BytesMut,
    trailers: Option<HeaderMap>,
}

impl BufferedBody {
    /// If there is a trailers frame buffered, returns a reference to it.
    /// Returns `None` if the body contained no trailers.
    pub fn trailers(&self) -> Option<&HeaderMap> {
        self.trailers.as_ref()
    }

    pub(crate) fn push_frame<B>(&mut self, frame: Frame<B>)
        where B: Buf {
        let frame = match frame.into_data() {
            Ok(mut data) => {
                // Only push this frame if it has some data in it, to avoid crashing on
                // `BufList::push`.
                while data.has_remaining() {
                    // Append the data to the buffer.
                    self.bufs.extend(data.chunk());
                    data.advance(data.remaining());
                }
                return;
            }
            Err(frame) => frame,
        };

        if let Ok(trailers) = frame.into_trailers() {
            if let Some(current) = &mut self.trailers {
                current.extend(trailers);
            } else {
                self.trailers = Some(trailers);
            }
        };
    }
}

impl Body for BufferedBody {
    type Data = BytesMut;
    type Error = Infallible;

    fn poll_frame(mut self: Pin<&mut Self>, _: &mut Context<'_>)
        -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = if self.bufs.len() > 0 /* Shall we skip this frame if body is empty? */ {
            let frame = Frame::data(self.bufs.to_owned());
            self.bufs.clear();
            frame
        } else if let Some(trailers) = self.trailers.take() {
            Frame::trailers(trailers)
        } else {
            return Poll::Ready(None);
        };

        Poll::Ready(Some(Ok(frame)))
    }
}

#[derive(Clone)]
pub struct RisuServer {
    configuration: RisuConfiguration,
    cache: Arc<ShardedCache<u128, Response<BufferedBody>>>,
    //client: Client<HttpConnector>,
}

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
                if let Err(err) = http2::Builder::new(TokioExecutor)
                    .serve_connection(io, server).await {
                    println!("Failed to serve connection: {:?}", err);
                }
            });
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

impl RisuServer {
    pub async fn call_async(&self, request: Request<Incoming>) -> Result<Response<BufferedBody>, Infallible> {

        let server = self.clone();

        let key_factory = |request: &Request<BufferedBody>| {
            // Hash request content
            let mut hasher = GxHasher::with_seed(123);
            request.uri().path().hash(&mut hasher);
            request.uri().query().hash(&mut hasher);
            request.body().bufs.hash(&mut hasher); // Todo: Make this more seamless
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
            let (mut sender, conn) = hyper::client::conn::http2::handshake(TokioExecutor, io).await.expect("Handshake failed");
    
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
            let forwarded_req = forwarded_req
                .body(body)
                .expect("Failed building request");

            debug!("Forwarding request");

            // Await the response...
            let res: Response<Incoming> = sender.send_request(forwarded_req).await.expect("Failed to send request");

            // Buffer response body so that we can cache it and return it
            let (parts, body) = res.into_parts();
            let collected = collect_buffered(body).await.unwrap();

            debug!("Received response from target with status: {:?}", parts.status);

            Ok(Response::from_parts(parts, collected))
        };

        let (parts, body) = request.into_parts();
        let buffered_body = collect_buffered(body).await.unwrap();
        let request = Request::from_parts(parts, buffered_body);

        let result: Result<Arc<Response<BufferedBody>>, ()> = server
            .cache
            .get_or_add_from_item2(request, key_factory, value_factory)
            .await;

        match result {
            Ok(response) => {
                let response = response.as_ref();
                let response: Response<BufferedBody> = response.clone();
                debug!("Received response from target with status: {:?}", response);
                return Ok(response);
            }
            Err(_) => panic!(),
        }
    }
}