# memds

A reimplementation of Redis for fun.

## Contributing Guide

### Test
Run test with:
```bash
cargo test
```

### Benchmark

1. run memds:
  ```bash
  RUST_LOG=error cargo run --release
  ```
  memds is currently hardcoded to run on port 6901

1. run redis-benchmark:
  with pipelining:
  ```bash
  redis-benchmark -t set,get -n 1000000 -r 1000000 -p 6901 -P 30
  ```
  without pipelining:
  ```bash
  redis-benchmark -t set,get -n 1000000 -r 1000000 -p 6901
  ```

