#!/usr/bin/env bash
set -Eeuo pipefail

NETWORK="${1:-testnet}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOYMENT="$ROOT/deployments/$NETWORK.json"
[[ -f "$DEPLOYMENT" ]] || { echo "verify-deployment: missing $DEPLOYMENT" >&2; exit 2; }

for cmd in git jq make python3 rustc stellar sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "verify-deployment: missing required command: $cmd" >&2
    exit 2
  }
done

CONTRACT_ID="$(jq -er '.contract_id | strings | select(length > 0)' "$DEPLOYMENT")"
SOURCE_COMMIT="$(jq -er '.source_commit | strings | select(length > 0)' "$DEPLOYMENT")"
RPC_URL="$(jq -er '.rpc_url | strings | select(length > 0)' "$DEPLOYMENT")"
PASSPHRASE="$(jq -er '.network_passphrase | strings | select(length > 0)' "$DEPLOYMENT")"
EXPECTED_HASH="$(jq -er '.wasm_hash | strings | select(length > 0)' "$DEPLOYMENT")"
STELLAR_VERSION="$(stellar --version | sed -n '1p')"
RUSTC_VERSION="$(rustc --version)"
python3 "$ROOT/scripts/verify_deployment.py" check-toolchain \
  "$DEPLOYMENT" "$RUSTC_VERSION" "$STELLAR_VERSION"

git -C "$ROOT" cat-file -e "$SOURCE_COMMIT^{commit}" || {
  echo "verify-deployment: source commit $SOURCE_COMMIT is not available locally; fetch full history" >&2
  exit 2
}

TEMP_DIR="$(mktemp -d)"
WORKTREE="$TEMP_DIR/source"
cleanup() {
  local status=$?
  trap - EXIT
  if [[ -d "$WORKTREE" ]]; then
    git -C "$ROOT" worktree remove --force "$WORKTREE" \
      || echo "verify-deployment: warning: could not remove worktree $WORKTREE" >&2
  fi
  rm -rf "$TEMP_DIR"
  exit "$status"
}
trap cleanup EXIT

echo "==> Fetching deployed WASM for $CONTRACT_ID ($NETWORK)"
stellar contract fetch --id "$CONTRACT_ID" --out-file "$TEMP_DIR/on-chain.wasm" \
  --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE"
ON_CHAIN_HASH="$(sha256sum "$TEMP_DIR/on-chain.wasm" | cut -d' ' -f1)"

echo "==> Building recorded source commit $SOURCE_COMMIT"
git -C "$ROOT" worktree add --detach "$WORKTREE" "$SOURCE_COMMIT"
WORKTREE_RUSTC="$(cd "$WORKTREE" && rustc --version)"
python3 "$ROOT/scripts/verify_deployment.py" check-toolchain \
  "$DEPLOYMENT" "$WORKTREE_RUSTC" "$STELLAR_VERSION"
(cd "$WORKTREE" && make -s build-optimized)
BUILT_HASH="$(sha256sum "$WORKTREE/target/stellar/indexer_contract.wasm" | cut -d' ' -f1)"

python3 "$ROOT/scripts/verify_deployment.py" compare-hash "$EXPECTED_HASH" "$ON_CHAIN_HASH"
python3 "$ROOT/scripts/verify_deployment.py" compare-hash "$EXPECTED_HASH" "$BUILT_HASH"
