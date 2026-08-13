# Repository Audit

## Current build state

- `cargo fmt -- --check` passes.
- `cargo clippy --all-targets --locked -- -D warnings` passes after typed event migration.
- `cargo test --locked` passes with 43 unit tests.
- `cargo build --target wasm32v1-none --release --locked` passes.
- `stellar-cli` is not installed locally; WASM optimization and deployment are therefore not locally verified.

## Completed improvements

- Added a committed `Cargo.lock` for reproducible dependency resolution.
- Added `rust-toolchain.toml` with stable Rust, `rustfmt`, `clippy`, and `wasm32v1-none`.
- Added a Makefile with standard local commands.
- Migrated the WASM build target from `wasm32-unknown-unknown` to `wasm32v1-none`.
- Replaced deprecated event publishing with typed `#[contractevent]` events.
- Added bounded and format-aware validation for folder names, IPFS CIDs, and template metadata URIs.
- Rejected duplicate template IDs.
- Explicitly rejected multiple Folder claims from one address.
- Added exact contract-error assertions across the unit tests.
- Added coverage for missing read paths, initialization guards, admin edge cases, and validation boundaries.

## Remaining gaps

- `stellar-cli` installation remains slow because it compiles a large dependency graph from source.
- Deployment cannot be fully verified without a funded Stellar account and network credentials.
- CID validation should eventually validate full CIDv0/CIDv1 structure rather than prefix only.
- Event payload compatibility should be coordinated with frontend/backend indexers before production rollout.
