.PHONY: help fmt fmt-check cargo-check clippy test build-wasm optimize-wasm check clean

help:
	@echo "Available targets:"
	@echo "  fmt           Format Rust sources"
	@echo "  fmt-check     Check Rust formatting"
	@echo "  cargo-check   Type-check the contract"
	@echo "  clippy        Run clippy with warnings denied"
	@echo "  test          Run unit tests"
	@echo "  build-wasm    Build release WASM"
	@echo "  optimize-wasm Optimize release WASM with Stellar CLI"
	@echo "  check         Run local pre-PR gates"
	@echo "  clean         Remove Cargo build artifacts"

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

cargo-check:
	cargo check --locked

clippy:
	cargo clippy --all-targets --locked -- -D warnings

test:
	cargo test --locked

build-wasm:
	cargo build --target wasm32v1-none --release --locked

optimize-wasm: build-wasm
	stellar contract optimize --wasm target/wasm32v1-none/release/indexer_contract.wasm

check: fmt-check cargo-check clippy test build-wasm

clean:
	cargo clean
