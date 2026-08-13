# Development Workflow

## Required tools

- Rust stable, managed by `rust-toolchain.toml`
- `make`
- Stellar CLI for `make optimize-wasm` and deployment

## Common commands

```bash
make fmt
make fmt-check
make cargo-check
make clippy
make test
make build-wasm
make check
```

`make check` is the local pre-PR gate. It intentionally does not run `make optimize-wasm` because Stellar CLI installation can be slow and deployment-specific.
