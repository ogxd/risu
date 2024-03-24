# risu

```shell
brew install protobuf
brew install grpcurl
cargo run --bin demo
cargo run --bin risu
grpcurl -plaintext -import-path ./proto -proto hello.proto -d '{"name": "Tonic"}' '127.0.0.1:3001' helloworld.Greeter/SayHello
```