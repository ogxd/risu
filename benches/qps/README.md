# QPS Benchmarks

```shell
./risu > cargo bench --bench qps
./risu > grpcurl -plaintext -import-path ./proto -proto hello.proto -d '{"name": "Tonic"}' '127.0.0.1:3001' helloworld.Greeter/SayHello
./risu > k6 run --vus 100 --duration 30s --rps 10000 benches/qps/k6.js
```