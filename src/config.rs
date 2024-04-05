use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct RisuConfiguration {
    #[serde(default = "default_in_memory_shards")]
    pub in_memory_shards: u16,

    #[serde(default = "default_cache_resident_size")]
    pub cache_resident_size: usize,

    #[serde(default = "default_cache_probatory_size")]
    pub cache_probatory_size: usize,

    #[serde(default = "default_listening_port")]
    pub listening_port: u16,

    #[serde(default = "default_target_addresses")]
    pub target_addresses: Vec<String>,

    #[serde(default = "default_http2")]
    pub http2: bool,
}

// https://github.com/serde-rs/serde/issues/368 🙄
fn default_in_memory_shards() -> u16 {
    8
}
fn default_cache_resident_size() -> usize {
    100_000
}
fn default_cache_probatory_size() -> usize {
    1_000_000
}
fn default_listening_port() -> u16 {
    3001
}
fn default_target_addresses() -> Vec<String> {
    vec!["127.0.0.1:3002".into()]
}
fn default_http2() -> bool {
    true
}
