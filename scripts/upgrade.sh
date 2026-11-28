#!/usr/bin/env bash
# upgrade.sh — upload a new wasm and upgrade an existing Folder contract.
#
# Usage: ./scripts/upgrade.sh <contract_id> [testnet|mainnet]
#
# Environment (all optional):
#   STELLAR_IDENTITY   Identity that pays for the upload (default: folder-deployer).
#   ADMIN_IDENTITY     Identity that signs `upgrade`; must be an admin of the
#                      contract (default: $STELLAR_IDENTITY).
#   STELLAR_RPC_URL    RPC endpoint (default: the network's public RPC).
#
# Storage is preserved by the upgrade; only the code changes. Exits non-zero on
# the first failure.
set -Eeuo pipefail
trap 'echo "upgrade.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

CONTRACT_ID="${1:-}"
NETWORK="${2:-testnet}"
if [[ -z "$CONTRACT_ID" ]]; then
  echo "Usage: $0 <contract_id> [testnet|mainnet]" >&2
  exit 1
fi
case "$NETWORK" in
  testnet)
    DEFAULT_RPC_URL="https://soroban-testnet.stellar.org"
    PASSPHRASE="Test SDF Network ; September 2015"
    ;;
  mainnet)
    DEFAULT_RPC_URL="https://mainnet.sorobanrpc.com"
    PASSPHRASE="Public Global Stellar Network ; September 2015"
    ;;
  *)
    echo "Unsupported network: $NETWORK. Expected 'testnet' or 'mainnet'." >&2
    exit 1
    ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IDENTITY="${STELLAR_IDENTITY:-folder-deployer}"
ADMIN_IDENTITY="${ADMIN_IDENTITY:-$IDENTITY}"
RPC_URL="${STELLAR_RPC_URL:-$DEFAULT_RPC_URL}"
WASM="target/stellar/indexer_contract.wasm"
NET_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")

die() { echo "upgrade.sh: $*" >&2; exit 1; }
for cmd in stellar cargo make sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

echo "==> Building optimized WASM"
make -s build-optimized >/dev/null
LOCAL_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
echo "    local wasm sha256: $LOCAL_HASH"

echo "==> Uploading WASM to $NETWORK"
WASM_HASH="$(stellar contract upload --wasm "$WASM" --source "$IDENTITY" "${NET_ARGS[@]}")"
[[ "$WASM_HASH" == "$LOCAL_HASH" ]] || die "uploaded wasm hash $WASM_HASH != local $LOCAL_HASH"

echo "==> Upgrading $CONTRACT_ID"
stellar contract invoke --id "$CONTRACT_ID" --source "$ADMIN_IDENTITY" "${NET_ARGS[@]}" -- \
  upgrade --admin "$(stellar keys address "$ADMIN_IDENTITY")" --new_wasm_hash "$WASM_HASH"

echo "==> Contract version"
stellar contract invoke --id "$CONTRACT_ID" "${NET_ARGS[@]}" -- version
