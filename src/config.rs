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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization() {
        let conf = "in_memory_shards: 42\n\
                    cache_resident_size: 123\n\
                    cache_probatory_size: 456\n\
                    listening_port: 789\n\
                    target_addresses: [ 1.2.3.4:1234, 5.6.7.8:5678 ]\n\
                    http2: false";

        let configuration: RisuConfiguration = serde_yaml::from_str::<RisuConfiguration>(conf).unwrap();
        
        assert_eq!(configuration.in_memory_shards, 42);
        assert_eq!(configuration.cache_resident_size, 123);
        assert_eq!(configuration.cache_probatory_size, 456);
        assert_eq!(configuration.listening_port, 789);
        assert_eq!(configuration.target_addresses, vec!["1.2.3.4:1234", "5.6.7.8:5678"]);
    }
}