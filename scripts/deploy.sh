#!/usr/bin/env bash
# deploy.sh — build, optimize, and deploy the Folder contract to Stellar testnet/mainnet
# Usage: ./scripts/deploy.sh [testnet|mainnet]
set -euo pipefail

NETWORK="${1:-testnet}"

echo "==> Building WASM..."
cargo build --target wasm32-unknown-unknown --release

echo "==> Optimizing WASM..."
stellar contract optimize \
  --wasm target/wasm32-unknown-unknown/release/indexer_contract.wasm

echo "==> Deploying to $NETWORK..."
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/indexer_contract.optimized.wasm \
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
