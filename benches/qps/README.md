# QPS Benchmarks

```shell
./risu > cargo bench --bench qps
./risu > grpcurl -plaintext -import-path ./proto -proto hello.proto -d '{"name": "Tonic"}' '127.0.0.1:3001' helloworld.Greeter/SayHello
./risu > k6 run benches/qps/k6.js
./risu > ghz --insecure --async --proto ./proto/hello.proto --call helloworld.Greeter/SayHello -c 100 -n 100000 --rps 10000 -d '{"name":"{{.WorkerID}}"}' 127.0.0.1:3001
```