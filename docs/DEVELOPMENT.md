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
make script-check
make snapshot-list
make check
```

`make check` is the local pre-PR gate. It runs formatting, `cargo check`, clippy, tests, release WASM build, and shell syntax validation. It intentionally does not run `make optimize-wasm` because Stellar CLI installation can be slow and deployment-specific.

## Test snapshots

Soroban auth snapshots under `test_snapshots/` are intentionally tracked. They make authorization behavior reviewable when contract tests change.
