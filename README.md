# Shared crates for the shine project

Some common crates for the shine project:
- shine-test
- shine-test-macros
- shine-macros
- shine-service
  
## shine-test

Automatically initializing logging and other handy features for the shine engine tests.

This crate was highly inspired by the [test-log](https://crates.io/crates/test-log) crate.

### Requirements

- rustls requires some other dependencies and it may result in `aws-lc-sys` compile errors
  - <https://medium.com/@rrnazario/rust-how-to-fix-failed-to-run-custom-build-command-for-aws-lc-sys-on-windows-c3bd2405ac6f>
  - https://github.com/rustls/rustls/issues/1913

## shine-service

The common features for all the server projects.

### Testing

```shell
# Start up mocked resource
$ docker compose up --build

# Run tests
$ cargo test -p shine-service
```

## Telemetry

### **Jaeger**

TBD: It was not tested since the deprecation of the opentelemetry-jaeger crate, see: <https://github.com/open-telemetry/opentelemetry-rust/issues/995>

Launch the application:
```shell
# Run jaeger in background with OTLP ingestion enabled.
$ docker run -d -p16686:16686 -p4317:4317 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest

# View spans
$ firefox http://localhost:16686/
```

# Cargo extensions

These are the most frequently used cargo extensions in the shine project:

```shell
cargo install cargo-outdated
cargo install cargo-tree
cargo install trunk
```