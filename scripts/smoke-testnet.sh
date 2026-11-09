#!/usr/bin/env bash
# smoke-testnet.sh — end-to-end check against the deployed testnet contract.
#
# Creates a fresh Friendbot-funded identity, then: claim_folder ->
# mint_portfolio -> update_portfolio -> verify_portfolio (submitted as a
# transaction so it has a hash), checking on-chain state after each step.
#
# Usage: ./scripts/smoke-testnet.sh
# Env:   DEPLOYMENT (default deployments/testnet.json)
#        SMOKE_IDENTITY (default smoke-<unix time>; created if missing)
#        SMOKE_MARKDOWN=1 prints a Markdown report instead of plain text.
set -Eeuo pipefail
trap 'echo "smoke-testnet.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DEPLOYMENT="${DEPLOYMENT:-deployments/testnet.json}"
for cmd in stellar jq python3; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "missing required command: $cmd" >&2; exit 1; }
done
[[ -f "$DEPLOYMENT" ]] || { echo "deployment file not found: $DEPLOYMENT" >&2; exit 1; }

NETWORK="$(jq -r .network "$DEPLOYMENT")"
RPC_URL="$(jq -r .rpc_url "$DEPLOYMENT")"
PASSPHRASE="$(jq -r .network_passphrase "$DEPLOYMENT")"
CONTRACT_ID="$(jq -r .contract_id "$DEPLOYMENT")"
TEMPLATE_ID="$(jq -r '.templates[0].template_id' "$DEPLOYMENT")"
[[ "$NETWORK" == "testnet" ]] || { echo "refusing to smoke-test network '$NETWORK'" >&2; exit 1; }
[[ "$TEMPLATE_ID" =~ ^[0-9]+$ ]] || { echo "no registered template in $DEPLOYMENT" >&2; exit 1; }

STAMP="$(date -u +%s)"
IDENTITY="${SMOKE_IDENTITY:-smoke-$STAMP}"
FOLDER="smoke-$STAMP.folder"
NET_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
EXPLORER="https://stellar.expert/explorer/$NETWORK"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
MD="${SMOKE_MARKDOWN:-0}"

fail() { echo "smoke-testnet.sh: $*" >&2; exit 1; }
say() { echo "$@"; }

# Content-addressed portfolio documents so the anchored CIDs are real CIDs
# of real bytes (pin these files to make them retrievable).
portfolio_cid() {
  local version="$1" file="$WORK/portfolio-v$1.json"
  jq -n --arg folder "$FOLDER" --argjson v "$version" --arg at "$STAMP" \
    '{schema_version: 1, folder: $folder, revision: $v, created_at_unix: ($at|tonumber), bio: "Folder testnet smoke run"}' >"$file"
  python3 -I scripts/template_cids.py "$file" | cut -d' ' -f1
}

# Command substitution runs in a subshell, so the last tx hash is passed
# through a file rather than a variable.
last_tx() { cat "$WORK/last_tx" 2>/dev/null || true; }
invoke() {
  local err="$WORK/stderr" out
  if ! out="$(stellar contract invoke --id "$CONTRACT_ID" --source "$IDENTITY" "${NET_ARGS[@]}" "$@" 2>"$err")"; then
    cat "$err" >&2
    fail "invoke $* failed"
  fi
  { grep -oE '(Signing transaction: |tx/)[0-9a-f]{64}' "$err" | tail -1 | grep -oE '[0-9a-f]{64}' || true; } >"$WORK/last_tx"
  printf '%s' "$out"
}
read_only() { invoke --send=no -- "$@"; }

STEP=0
report() {
  local name="$1" tx="$2" result="$3"
  STEP=$((STEP + 1))
  [[ -n "$tx" ]] || fail "$name did not produce a transaction hash"
  if [[ "$MD" == "1" ]]; then
    say "| $STEP | \`$name\` | $result | [\`$tx\`]($EXPLORER/tx/$tx) |"
  else
    say "[$STEP] $name: $result"
    say "    tx: $tx"
    say "    $EXPLORER/tx/$tx"
  fi
}

if ! stellar keys address "$IDENTITY" >/dev/null 2>&1; then
  stellar keys generate "$IDENTITY" "${NET_ARGS[@]}" --fund >/dev/null 2>&1 \
    || fail "could not create/fund identity $IDENTITY via Friendbot"
fi
USER_ADDR="$(stellar keys address "$IDENTITY")"
CID_V1="$(portfolio_cid 1)"
CID_V2="$(portfolio_cid 2)"

if [[ "$MD" == "1" ]]; then
  say "- Network: \`$NETWORK\` (\`$RPC_URL\`)"
  say "- Contract: [\`$CONTRACT_ID\`]($EXPLORER/contract/$CONTRACT_ID)"
  say "- Test account: [\`$USER_ADDR\`]($EXPLORER/account/$USER_ADDR) (fresh, Friendbot-funded)"
  say "- Folder: \`$FOLDER\`, template: \`$TEMPLATE_ID\`"
  say "- Portfolio CIDs: v1 \`$CID_V1\`, v2 \`$CID_V2\`"
  say ""
  say "| # | Call | Result | Transaction |"
  say "|---|---|---|---|"
else
  say "network:  $NETWORK ($RPC_URL)"
  say "contract: $CONTRACT_ID"
  say "user:     $USER_ADDR"
  say "folder:   $FOLDER"
fi

# 1. claim_folder
invoke -- claim_folder --owner "$USER_ADDR" --folder_name "$FOLDER" >/dev/null
TX="$(last_tx)"
OWNER="$(read_only get_folder --folder_name "$FOLDER" | jq -r .)"
[[ "$OWNER" == "$USER_ADDR" ]] || fail "get_folder returned $OWNER"
report claim_folder "$TX" "get_folder(\`$FOLDER\`) = owner"

# 2. mint_portfolio
invoke -- mint_portfolio --owner "$USER_ADDR" --template_id "$TEMPLATE_ID" --ipfs_cid "$CID_V1" >/dev/null
TX="$(last_tx)"
NFT="$(read_only get_portfolio --owner "$USER_ADDR")"
[[ "$(jq -r .ipfs_cid <<<"$NFT")" == "$CID_V1" ]] || fail "minted CID mismatch: $NFT"
[[ "$(jq -r .template_id <<<"$NFT")" == "$TEMPLATE_ID" ]] || fail "minted template mismatch: $NFT"
report mint_portfolio "$TX" "ipfs_cid = v1, template_id = $TEMPLATE_ID"

# 3. update_portfolio
invoke -- update_portfolio --owner "$USER_ADDR" --ipfs_cid "$CID_V2" >/dev/null
TX="$(last_tx)"
NFT="$(read_only get_portfolio --owner "$USER_ADDR")"
[[ "$(jq -r .ipfs_cid <<<"$NFT")" == "$CID_V2" ]] || fail "updated CID mismatch: $NFT"
report update_portfolio "$TX" "ipfs_cid = v2"

# 4. verify_portfolio — read-only, submitted anyway so the run has a hash.
VERIFIED="$(invoke --send=yes -- verify_portfolio --owner "$USER_ADDR")"
TX="$(last_tx)"
[[ "$VERIFIED" == "true" ]] || fail "verify_portfolio returned $VERIFIED"
report verify_portfolio "$TX" "returned \`true\`"

if [[ "$MD" != "1" ]]; then
  say "OK: smoke test passed (4 transactions)"
fi
