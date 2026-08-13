# Repository Audit

## Current build state

- `cargo fmt -- --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes after typed event migration.
- `cargo test --locked` passes with 17 unit tests.
- `cargo build --target wasm32v1-none --release --locked` passes.
- `stellar-cli` is not installed locally; WASM optimization and deployment are therefore not locally verified.

## Completed improvements

- Added a committed `Cargo.lock` for reproducible dependency resolution.
- Added `rust-toolchain.toml` with stable Rust, `rustfmt`, `clippy`, and `wasm32v1-none`.
- Added a Makefile with standard local commands.
- Migrated the WASM build target from `wasm32-unknown-unknown` to `wasm32v1-none`.
- Replaced deprecated event publishing with typed `#[contractevent]` events.
- Added bounded validation for folder names, IPFS CIDs, and template metadata URIs.

## Remaining gaps

- `stellar-cli` installation remains slow because it compiles a large dependency graph from source.
- Deployment cannot be fully verified without a funded Stellar account and network credentials.
- Tests should assert exact contract errors instead of broad `is_err()` checks.
- Folder name validation should eventually enforce allowed characters and suffix policy.
- CID validation should eventually validate CIDv0/CIDv1 structure rather than length only.
- Template registration currently overwrites an existing template ID.
- Users can claim only one folder by storage shape, but the contract should explicitly reject a second claim.
- Admin operations need broader tests for duplicate add, unknown remove, and removed admin behavior.
- CI should avoid installing obsolete Stellar CLI features.
