use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct RisuConfiguration
{
    #[serde(default = "default_in_memory_shards")]
    pub in_memory_shards: u16,

    #[serde(default = "default_cache_resident_size")]
    pub cache_resident_size: usize,

    #[serde(default = "default_cache_probatory_size")]
    pub cache_probatory_size: usize,

    #[serde(default = "default_cache_ttl_seconds")]
    pub cache_ttl_seconds: usize,

    #[serde(default = "default_listening_port")]
    pub listening_port: u16,

    #[serde(default = "default_http2")]
    pub http2: bool,

    #[serde(default = "default_prometheus_port")]
    pub prometheus_port: u16,

    #[serde(default = "default_healthcheck_port")]
    pub healthcheck_port: u16,

    #[serde(default = "default_max_idle_connections_per_host")]
    pub max_idle_connections_per_host: u16,
}

// https://github.com/serde-rs/serde/issues/368 🙄
fn default_in_memory_shards() -> u16
{
    8
}
fn default_cache_resident_size() -> usize
{
    100_000
}
fn default_cache_probatory_size() -> usize
{
    1_000_000
}
fn default_cache_ttl_seconds() -> usize
{
    600
}
fn default_listening_port() -> u16
{
    3001
}
fn default_http2() -> bool
{
    true
}
fn default_prometheus_port() -> u16
{
    8000
}
fn default_healthcheck_port() -> u16
{
    8001
}
fn default_max_idle_connections_per_host() -> u16
{
    4
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_config_deserialization()
    {
        let conf = "in_memory_shards: 42\n\
                    cache_resident_size: 123\n\
                    cache_probatory_size: 456\n\
                    listening_port: 789\n\
                    http2: false";

        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(conf).unwrap();

        assert_eq!(configuration.in_memory_shards, 42);
        assert_eq!(configuration.cache_resident_size, 123);
        assert_eq!(configuration.cache_probatory_size, 456);
        assert_eq!(configuration.listening_port, 789);
    }
}
