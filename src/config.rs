use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct RisuConfiguration
{
    pub in_memory_shards: u16,
    pub cache_resident_size: usize,
    pub cache_probatory_size: usize,
    pub cache_ttl_seconds: usize,
    pub listening_port: u16,
    pub http2: bool,
    pub prometheus_port: Option<u16>,
    pub healthcheck_port: Option<u16>,
    pub max_idle_connections_per_host: u16,
}

impl Default for RisuConfiguration {
    fn default() -> Self {
        Self {
            in_memory_shards: 8,
            cache_resident_size: 100_000,
            cache_probatory_size: 1_000_000,
            cache_ttl_seconds: 600,
            listening_port: 3001,
            http2: true,
            prometheus_port: None,
            healthcheck_port: None,
            max_idle_connections_per_host: 4
        }
    }
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
