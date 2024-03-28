# risu

```shell
brew install protobuf
brew install grpcurl
cargo run --bin demo
cargo run --bin risu
grpcurl -plaintext -import-path ./proto -proto hello.proto -d '{"name": "Tonic"}' '127.0.0.1:3001' helloworld.Greeter/SayHello
```

## Todo

- [ ] Setup a way to test risu against various targets
- [ ] Support HTTP/1.1
- [ ] Support HTTP/2
- [ ] Support https
- [ ] Properly route
- [ ] Expose prometheus metrics
- [ ] Setup and run benchmarks
- [ ] Setup CI
- [ ] Implement LRU cache
- [ ] Implement probatory LRU cache
- [ ] Implemented in-memory sharding
- [ ] Design hot keys cluster sharing
- [ ] Find out how risu will know service to reach
- [ ] Find out how to configure service