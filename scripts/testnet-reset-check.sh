#!/usr/bin/env bash
# testnet-reset-check.sh — report whether the recorded testnet contract exists.
#
# Usage: ./scripts/testnet-reset-check.sh
# Env:   DEPLOYMENT   deployment record (default deployments/testnet.json)
#        CONTRACT_ID  override the contract ID from the record
#        STELLAR      stellar CLI binary (default: stellar)
#
# Prints exists=true|false (also appended to $GITHUB_OUTPUT when set).
# Exit codes: 0 = check completed, 2 = the RPC/CLI failed for another reason.
set -Euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DEPLOYMENT="${DEPLOYMENT:-deployments/testnet.json}"
STELLAR="${STELLAR:-stellar}"
CONTRACT_ID="${CONTRACT_ID:-$(jq -r .contract_id "$DEPLOYMENT")}"
RPC_URL="$(jq -r .rpc_url "$DEPLOYMENT")"
PASSPHRASE="$(jq -r .network_passphrase "$DEPLOYMENT")"

if out="$("$STELLAR" contract info interface --id "$CONTRACT_ID" --network testnet \
  --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" 2>&1)"; then
  exists=true
elif grep -qiE 'not found|does not exist|missing|no such' <<<"$out"; then
  exists=false
else
  echo "contract check failed for $CONTRACT_ID:" >&2
  echo "$out" >&2
  exit 2
fi

echo "contract_id=$CONTRACT_ID"
echo "exists=$exists"
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  { echo "contract_id=$CONTRACT_ID"; echo "exists=$exists"; } >>"$GITHUB_OUTPUT"
fi
