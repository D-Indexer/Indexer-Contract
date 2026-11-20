.PHONY: help fmt fmt-check cargo-check clippy test build-wasm build-optimized optimize-wasm wasm-hash script-check snapshot-list mutants check clean

STELLAR_CLI_VERSION ?= 28.1.0
WASM_OUT_DIR := target/stellar
OPTIMIZED_WASM := $(WASM_OUT_DIR)/indexer_contract.wasm

help:
	@echo "Available targets:"
	@echo "  fmt           Format Rust sources"
	@echo "  fmt-check     Check Rust formatting"
	@echo "  cargo-check   Type-check the contract"
	@echo "  clippy        Run clippy with warnings denied"
	@echo "  test          Run unit tests"
	@echo "  build-wasm    Build release WASM with cargo (unoptimized)"
	@echo "  build-optimized Build optimized WASM with Stellar CLI -> $(OPTIMIZED_WASM)"
	@echo "  wasm-hash     Print the sha256 (on-chain wasm hash) of the optimized WASM"
	@echo "  script-check  Validate shell scripts"
	@echo "  snapshot-list List tracked Soroban auth snapshots"
	@echo "  mutants       Run cargo-mutants with mutants.toml"
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

build-optimized:
	@command -v stellar >/dev/null || { echo "stellar CLI not found (expected $(STELLAR_CLI_VERSION))" >&2; exit 1; }
	stellar contract build --locked --optimize --out-dir $(WASM_OUT_DIR)
	@sha256sum $(OPTIMIZED_WASM)

# Kept for backwards compatibility with older docs.
optimize-wasm: build-optimized

wasm-hash: build-optimized
	@sha256sum $(OPTIMIZED_WASM) | cut -d' ' -f1

script-check:
	@for f in scripts/*.sh; do bash -n "$$f" || exit 1; done

snapshot-list:
	@find test_snapshots -type f | sort

mutants:
	@command -v cargo-mutants >/dev/null || { echo "cargo-mutants not installed (cargo install cargo-mutants)" >&2; exit 1; }
	cargo mutants --config mutants.toml

check: fmt-check cargo-check clippy test build-wasm script-check

clean:
	cargo clean
