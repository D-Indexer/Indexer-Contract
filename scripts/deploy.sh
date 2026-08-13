#!/usr/bin/env bash
# deploy.sh — build, optimize, and deploy the Folder contract to Stellar testnet/mainnet
# Usage: ./scripts/deploy.sh [testnet|mainnet]
set -euo pipefail

NETWORK="${1:-testnet}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd stellar

require_env() {
  if [[ -z "${!1:-}" ]]; then
    echo "Missing required environment variable: $1" >&2
    exit 1
  fi
}

require_env DEPLOYER_SECRET
require_env ADMIN_ADDRESS

echo "==> Building WASM..."
cargo build --target wasm32v1-none --release

echo "==> Optimizing WASM..."
stellar contract optimize \
  --wasm target/wasm32v1-none/release/indexer_contract.wasm

echo "==> Deploying to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/indexer_contract.optimized.wasm \
  --source "${DEPLOYER_SECRET}" \
  --network "$NETWORK")

echo "Contract deployed: $CONTRACT_ID"

echo "==> Initializing contract..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "${DEPLOYER_SECRET}" \
  --network "$NETWORK" \
  -- initialize \
  --admin "${ADMIN_ADDRESS}"

echo "Done. CONTRACT_ID=$CONTRACT_ID"
