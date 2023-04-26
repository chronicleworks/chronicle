# `export` feature

## Example

```bash
RUST_LOG=debug,cranelift_codegen=off,wasmtime_cranelift=off \
    cargo run --bin chronicle --features inmem,export -- serve-api
```
