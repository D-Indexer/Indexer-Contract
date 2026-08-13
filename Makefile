.PHONY: fmt fmt-check cargo-check clippy test build-wasm check clean

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

check: fmt-check cargo-check clippy test build-wasm

clean:
	cargo clean
