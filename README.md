# risu

Blazingly fast and ultra-efficient multi-protocol read-through caching proxy.

## Todo

- [x] Setup a way to test risu against various targets
- [x] Support HTTP/1.1
- [x] Support HTTP/2
- [ ] Support https
- [x] Properly route
- [x] Add basic logging
- [x] Expose prometheus metrics
- [x] Setup and run benchmarks
- [x] Setup CI
- [x] Implement arena-based linked list
- [x] Implement LRU cache
- [x] Implement probatory LRU cache
- [x] Implemented in-memory sharding
- [x] Use gxhash for sharding and keying
- [x] Implement actual caching in risu
- [ ] Design hot keys cluster sharing
- [x] Find out how risu will know service to reach
- [x] Find out how to configure service
- [ ] Experiment with bytedance/monoio