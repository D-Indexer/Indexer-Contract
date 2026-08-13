.PHONY: fmt fmt-check clippy test build-wasm check clean

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

build-wasm:
	cargo build --target wasm32v1-none --release

check: fmt-check clippy test build-wasm

clean:
	cargo clean
