use std::{str::FromStr, sync::Arc};

use hyper::{header::{HeaderName, HeaderValue}, Request, Response, Uri};
use serde::{Deserialize, Serialize};
use serde_inline_default::serde_inline_default;
use crate::{CallContext, buffered_body::BufferedBody, config::{CacheConfig, cache::CachedResponse, conditions::Condition}};
use reqwest as rw;

trait Middleware {
    async fn init(&mut self) {}
    async fn on_request(&self, _context: &mut CallContext, _request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> { None }
    async fn on_response(&self, _context: &mut CallContext, _request: &Request<BufferedBody>, _response: &mut Option<Response<BufferedBody>>, _when_applies: bool) {}
}

/// A middleware as it appears in the YAML config: a typed middleware kind plus
/// an optional `when:` predicate that gates both the request and response phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareEntry {
    #[serde(flatten)]
    pub kind: MiddlewareEnum,
    #[serde(default)]
    pub when: Option<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MiddlewareEnum {
    BlockRequest(Block),
    UseCache(CacheMiddleware), // TODO: Support HTTP cache headers, validation, etc.
    ForwardRequest(Forward), // TODO: Support multiple targets, DNS load balancing, redirects, etc.
    AddResponseHeader(AddHeader),
    ReplaceResponseBody(ReplaceResponseBody),
    Log(Log),
    // TODO: RateLimit, Compress,
}

impl MiddlewareEntry {
    pub async fn init(&mut self) {
        self.kind.init().await
    }

    pub async fn on_request(&self, context: &mut CallContext, request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> {
        debug!("Processing middleware request: {:?}", self.kind);
        if !self.when.as_ref().is_none_or(|w| w.evaluate(request)) {
            return None;
        }
        self.kind.on_request(context, request).await
    }

    pub async fn on_response(&self, context: &mut CallContext, request: &Request<BufferedBody>, response: &mut Option<Response<BufferedBody>>) {
        debug!("Processing middleware response: {:?}", self.kind);
        let when_applies = self.when.as_ref().is_none_or(|w| w.evaluate_with_response(request, response));
        self.kind.on_response(context, request, response, when_applies).await
    }
}

impl MiddlewareEnum {

    pub async fn init(&mut self) {
        match self {
            MiddlewareEnum::BlockRequest(m) => m.init().await,
            MiddlewareEnum::UseCache(m) => m.init().await,
            MiddlewareEnum::ForwardRequest(m) => m.init().await,
            MiddlewareEnum::AddResponseHeader(m) => m.init().await,
            MiddlewareEnum::ReplaceResponseBody(m) => m.init().await,
            MiddlewareEnum::Log(m) => m.init().await,
        }
    }

    pub async fn on_request(&self, context: &mut CallContext, request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> {
        match self {
            MiddlewareEnum::BlockRequest(m) => m.on_request(context, request).await,
            MiddlewareEnum::UseCache(m) => m.on_request(context, request).await,
            MiddlewareEnum::ForwardRequest(m) => m.on_request(context, request).await,
            MiddlewareEnum::AddResponseHeader(m) => m.on_request(context, request).await,
            MiddlewareEnum::ReplaceResponseBody(m) => m.on_request(context, request).await,
            MiddlewareEnum::Log(m) => m.on_request(context, request).await,
        }
    }

    pub async fn on_response(&self, context: &mut CallContext, request: &Request<BufferedBody>, response: &mut Option<Response<BufferedBody>>, when_applies: bool) {
        match self {
            MiddlewareEnum::BlockRequest(m) => m.on_response(context, request, response, when_applies).await,
            MiddlewareEnum::UseCache(m) => m.on_response(context, request, response, when_applies).await,
            MiddlewareEnum::ForwardRequest(m) => m.on_response(context, request, response, when_applies).await,
            MiddlewareEnum::AddResponseHeader(m) => m.on_response(context, request, response, when_applies).await,
            MiddlewareEnum::ReplaceResponseBody(m) => m.on_response(context, request, response, when_applies).await,
            MiddlewareEnum::Log(m) => m.on_response(context, request, response, when_applies).await,
        }
    }
}

#[serde_inline_default]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Block {
    #[serde_inline_default(403)]
    pub status_code: u16,
}

impl Middleware for Block {
    async fn on_request(&self, _context: &mut CallContext, _request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> {
        return Some(Response::builder().status(self.status_code).body(BufferedBody::from_body(b"Blocked by cacheus")).unwrap());
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheMiddleware {
    pub cache_name: String,
}

// - Stale
// [Cache::on_request] lookup: expired -> [Forward::on_request] failed -> [Forward::on_response] -> [Cache::on_response] no response: lookup: expired: mark stale
// - Miss
// [Cache::on_request] lookup: not found -> [Forward::on_request] -> [Forward::on_response] -> [Cache::on_response] response: insert
// [Cache::on_request] lookup: error -> [Forward::on_request] -> [Forward::on_response] -> [Cache::on_response] response: insert
// - Hit
// [Cache::on_request] lookup: found -> [Cache::on_response] response + hit context: no insert
impl Middleware for CacheMiddleware {
    async fn on_request(&self, context: &mut CallContext, request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> {
        if let Some((cache_config, cache_instance)) = context.caches.get(&self.cache_name) {
            let key = cache_config.create_key(&request);
            match cache_instance.get(&key).await {
                Err(e) => {
                    log::error!("Cache lookup error for key {:?}: {:?}", key, e);
                    // Error during lookup, treat as miss
                    context.variables.insert("$cache_status".to_string(), "miss".to_string());
                },
                Ok(cached_response) => match cached_response {
                    None => {
                        // True cache miss
                        context.variables.insert("$cache_status".to_string(), "miss".to_string());
                    },
                    Some(cached_response) => {
                        let current_epoch: u64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                        let ttl_seconds = match cache_config {
                            CacheConfig::InMemory { in_memory } => in_memory.ttl_seconds,
                            CacheConfig::Hybrid { hybrid } => hybrid.ttl_seconds
                        };
                        if current_epoch - cached_response.value().insertion_epoch > ttl_seconds {
                            // Stale entry! We keep it in case we need it later
                            context.stale_response.replace(cached_response.value().response.clone());
                            // Stale is considered a miss in the context of the a fresh lookup
                            context.variables.insert("$cache_status".to_string(), "miss".to_string());
                        }
                        else
                        {
                            // Cache hit!
                            context.variables.insert("$cache_status".to_string(), "hit".to_string());
                            return Some(cached_response.value().response.clone());
                        }
                    }
                }
            }
        }
        else
        {
            log::error!("No cache named '{}' found. Verify configuration.", self.cache_name);
            // Cache not found, treat as miss
            context.variables.insert("$cache_status".to_string(), "miss".to_string());
        }
        return None;
    }

    async fn on_response(&self, context: &mut CallContext, request: &Request<BufferedBody>, response: &mut Option<Response<BufferedBody>>, when_applies: bool) {
        // Stale fallback intentionally bypasses `when`: e.g. with `when: status_code_is: 200`,
        // a 404 from the origin would otherwise skip the cache entirely. We want to substitute
        // the stale entry first, then re-check `when` for the insert path.
        let has_response = response.is_some();
        if (!has_response || !when_applies) && context.stale_response.is_some() {
            context.variables.insert("$cache_status".to_string(), "stale".to_string());
            response.replace(context.stale_response.take().unwrap());
        }
        if !when_applies {
            return;
        }
        if has_response
        {
            // Response: either miss (fetched from subsequent middleware) or hit (taken from cache)
            // In case of miss, we insert the response into the cache
            if context.variables.get("$cache_status").map_or(false, |v| v == "hit") {
                return;
            }
            if let Some((cache_config, cache_instance)) = context.caches.get(&self.cache_name) {
                let key = cache_config.create_key(&request);
                let insertion_epoch: u64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                cache_instance.insert(key, CachedResponse { insertion_epoch: insertion_epoch, response: response.clone().unwrap() });
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Forward {
    pub target_host: String,
    pub enforce_http2: bool,
    pub enforce_http: bool,
    /// Nagle algorithm can cause delays in sending small packets.
    /// Default is disabled.
    pub use_nagle: bool,
    /// The maximum number of redirects to follow. Set 0 to disable redirects.
    /// Default is 3.
    pub max_redirects: u8,
    pub scheme: Option<String>,
    #[serde(skip)]
    pub client: Option<Arc<rw::Client>>,
}

impl Default for Forward {
    fn default() -> Self {
        Self {
            target_host: "".to_string(),
            enforce_http2: false,
            enforce_http: false,
            use_nagle: false,
            max_redirects: 3,
            scheme: None,
            client: None,
        }
    }
}

impl Middleware for Forward {

    async fn init(&mut self) {
        let mut builder = rw::Client::builder();

        if self.max_redirects > 0 {
            builder = builder.redirect(rw::redirect::Policy::limited(3));
        } else {
            builder = builder.redirect(rw::redirect::Policy::none());
        }

        builder = builder.tcp_nodelay(!self.use_nagle);

        // Try to honor HTTP/2 vs HTTP/1 preferences when specified.
        if self.enforce_http2 {
            builder = builder.http2_prior_knowledge();
        }
        if self.enforce_http {
            builder = builder.http1_only();
        }

        let client = builder
            .build()
            .expect("Failed to build reqwest client");

        self.client = Some(Arc::new(client));
    }

    async fn on_request(&self, context: &mut CallContext, request: &mut Request<BufferedBody>) -> Option<Response<BufferedBody>> {
        let target_host = match request.headers().get("x-target-host") {
            Some(value) => value.to_str().unwrap().to_string(),
            None => self.target_host.clone(),
        };

        if target_host.is_empty() {
            panic!("Missing X-Target-Host header! Can't forward the request.");
        }

        let target_uri = Uri::builder()
            .scheme(self.scheme.clone().unwrap_or("http".to_string()).as_str()) // Suboptimal
            .authority(target_host.clone())
            .path_and_query(request.uri().path_and_query().unwrap().clone())
            .build()
            .expect("Failed to build target URI");

        context.variables.insert("$forward_target".to_string(), target_uri.to_string());

        // Build reqwest request
        let url = target_uri.to_string();
        let client = self.client.as_ref().expect("Client not initialized");
        let mut req_builder = client.request(
            reqwest::Method::from_bytes(request.method().as_str().as_bytes()).unwrap(),
            &url,
        );

        // Copy headers, skipping hop-by-hop or ones managed by reqwest
        // - Skip "host"; reqwest sets it from URL
        // - Skip "accept-encoding" to avoid auto-compression issues
        // - Content-Length will be set by reqwest based on body
        for (name, value) in request.headers().iter() {
            let name_str = name.as_str();
            if name_str.eq_ignore_ascii_case("host") || name_str.eq_ignore_ascii_case("accept-encoding") || name_str.eq_ignore_ascii_case("content-length") {
                continue;
            }
            req_builder = req_builder.header(name, value);
        }

        // Attach body
        let body_bytes = request.body().body_bytes();
        if !body_bytes.is_empty() {
            req_builder = req_builder.body(body_bytes.to_vec());
        }

        let res = req_builder.send().await.expect("Failed to send request");

        // Map back to hyper::Response<BufferedBody>
        let status = res.status();
        let mut builder = Response::builder().status(status);

        // Headers
        {
            let headers_mut = builder.headers_mut().unwrap();
            for (k, v) in res.headers().iter() {
                // reqwest's HeaderName/Value are re-exported from http, compatible with hyper
                headers_mut.insert(k.clone(), v.clone());
            }
        }

        let bytes = res.bytes().await.expect("Failed to read response body");
        let body = BufferedBody::from_body(&bytes);
        let response = builder.body(body).expect("Failed to build response");
        return Some(response);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AddHeader {
    pub name: String,
    pub value: String,
}

impl Middleware for AddHeader {

    async fn on_response(&self, _context: &mut CallContext, _request: &Request<BufferedBody>, response: &mut Option<Response<BufferedBody>>, when_applies: bool) {
        if !when_applies {
            return;
        }
        if let Some(response) = response {
            // Replace variables in header value, if any
            let value = _context.variables.iter().fold(self.value.clone(), |acc, (k, v)| acc.replace(k.as_str(), v.as_str()));
            // Insert the header
            response.headers_mut().insert(
                HeaderName::from_str(self.name.as_str()).unwrap(), // Clearly not optimal
                HeaderValue::from_str(value.as_str()).unwrap());
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReplaceResponseBody {
    pub find: String,
    pub replacement: String,
}

impl Middleware for ReplaceResponseBody {

    async fn on_response(&self, _context: &mut CallContext, _request: &Request<BufferedBody>, response: &mut Option<Response<BufferedBody>>, when_applies: bool) {
        if !when_applies {
            return;
        }
        if let Some(response) = response {
            if let Some(content_type) = response.headers().get("content-type") {
                if content_type.to_str().unwrap().contains("json") {
                    let content_length = response.body_mut().replace_strings(&self.find, &self.replacement);
                    // Response length may have changed, so we need to update the content-length header
                    response.headers_mut().insert("content-length", content_length.to_string().parse().unwrap());
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Log {
    pub level: String,
    pub value: String,
}

impl Middleware for Log {

    async fn on_response(&self, _context: &mut CallContext, _request: &Request<BufferedBody>, _response: &mut Option<Response<BufferedBody>>, when_applies: bool) {
        if !when_applies {
            return;
        }
        // Replace variables in header value, if any
        let value = _context.variables.iter().fold(self.value.clone(), |acc, (k, v)| acc.replace(k.as_str(), v.as_str()));
        log::log!(log::Level::from_str(self.level.as_str()).unwrap_or(log::Level::Info), "{}", value);
    }
}

#[cfg(test)]
mod tests {
    use log::LevelFilter;
    use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};

    use super::*;

    #[tokio::test]
    async fn test_cache_middleware() {

        // Configure logging to print to console
        CombinedLogger::init(vec![TermLogger::new(
            LevelFilter::Debug,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        )])
        .unwrap();

        // Create middleware
        let middleware = CacheMiddleware {
            cache_name: "test_cache".to_string(),
        };

        // Create cache
        let mut caches = std::collections::HashMap::new();
        let cache_config = CacheConfig::InMemory { in_memory: crate::config::cache::MemoryCache {
                name: "test_cache".to_string(),
                ttl_seconds: 2,
                ..Default::default()
            }
        };
        cache_config.add_cache(&mut caches).await;
        assert!(caches.contains_key("test_cache"));

        // Create call context
        let mut call_context = CallContext {
            caches: Arc::new(caches),
            ..Default::default()
        };

        // Create request
        let mut request = Request::builder().uri("/test").body(BufferedBody::default()).unwrap();
        let mut remote_response = Some(Response::builder().status(200).body(BufferedBody::from_body(b"Hello, world!")).unwrap());

        // Call middleware first, expecting a miss
        let cache_response = middleware.on_request(&mut call_context, &mut request).await;
        middleware.on_response(&mut call_context, &request, &mut remote_response, true).await;

        assert!(cache_response.is_none());
        assert_eq!(call_context.variables.get("$cache_status"), Some(&"miss".to_string()));

        // Call middleware second, expecting a hit
        let mut cache_response = middleware.on_request(&mut call_context, &mut request).await;
        assert!(cache_response.is_some());
        middleware.on_response(&mut call_context, &request, &mut cache_response, true).await;

        assert_eq!(call_context.variables.get("$cache_status"), Some(&"hit".to_string()));

        // Wait for 3s, then entry should be stale
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let cache_response = middleware.on_request(&mut call_context, &mut request).await;
        let mut stale_response = None; // Simulate failed fetch
        middleware.on_response(&mut call_context, &request, &mut stale_response, true).await;

        assert!(cache_response.is_none(), "Expected no response from cache since entry is stale");
        assert!(stale_response.is_some(), "Expected stale response to be returned");
        assert_eq!(call_context.variables.get("$cache_status"), Some(&"stale".to_string()));

        // Call again, but now the remote call is successful, so we should cache the new response and get a miss
        let cache_response = middleware.on_request(&mut call_context, &mut request).await;
        middleware.on_response(&mut call_context, &request, &mut remote_response, true).await;

        assert!(cache_response.is_none(), "Expected no response from cache since entry is stale");
        assert_eq!(call_context.variables.get("$cache_status"), Some(&"miss".to_string()));
    }
}
