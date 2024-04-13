use prometheus::{Counter, Encoder, Histogram, HistogramOpts, Opts, Registry, TextEncoder};

pub struct Metrics
{
    pub response_time: Histogram,
    pub cache_calls: Counter,
    // cache_hits: Counter,
    pub cache_misses: Counter,
    // cache_evictions: Counter,
    // cache_resident_size: Counter,
    // cache_probatory_size: Counter,
    registry: Registry,
}

impl Metrics
{
    pub fn new() -> Metrics
    {
        let metrics = Metrics {
            response_time: Histogram::with_opts(HistogramOpts::new("response_time", "Response time").buckets(vec![
                0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1., 2., 5., 10., 20., 50., 100., 200., 500., 1000., 2000., 5000.,
                10000.,
            ]))
            .unwrap(),
            cache_calls: Counter::with_opts(Opts::new("cache_calls", "Number of cache calls")).unwrap(),
            cache_misses: Counter::with_opts(Opts::new("cache_misses", "Number of cache misses")).unwrap(),
            registry: Registry::new(),
        };
        metrics
            .registry
            .register(Box::new(metrics.response_time.clone()))
            .unwrap();
        metrics
            .registry
            .register(Box::new(metrics.cache_calls.clone()))
            .unwrap();
        metrics
            .registry
            .register(Box::new(metrics.cache_misses.clone()))
            .unwrap();
        metrics
    }

    pub fn encode(&self) -> Vec<u8>
    {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        buffer
    }
}
