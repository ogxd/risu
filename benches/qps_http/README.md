# QPS Benchmarks

```shell
./risu > cargo bench --bench qps
./risu > curl http://127.0.0.1:3001
./risu > k6 run --out dashboard benches/qps_http/k6.js
```