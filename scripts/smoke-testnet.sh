#!/usr/bin/env bash
# smoke-testnet.sh — end-to-end check against the deployed testnet contract.
#
# Creates a fresh Friendbot-funded identity, then: claim_folder ->
# mint_portfolio -> update_portfolio -> verify_portfolio (submitted as a
# transaction so it has a hash), checking on-chain state after each step.
#
# Usage: ./scripts/smoke-testnet.sh [--simulate]
# Env:   DEPLOYMENT (default deployments/testnet.json)
#        SMOKE_IDENTITY (default smoke-<unix time>; created if missing)
#        SMOKE_MARKDOWN=1 prints a Markdown report instead of plain text.
#
# --simulate: dry run. Every step is simulated (`--send=no`); nothing is
# signed or submitted, and the source account's sequence number is checked to
# be unchanged. Required for (and the only mode allowed on) mainnet.
# Simulated steps cannot see each other's writes, so each one runs against an
# account whose preconditions already hold and read-back assertions are
# skipped:
#   claim_folder      owner = a fresh, unfunded keypair (owns no folder)
#   mint_portfolio    owner = $SIM_MINT_OWNER (owns a folder, no portfolio)
#   update_portfolio  owner = $SIM_PORTFOLIO_OWNER (owns a portfolio)
#   verify_portfolio  owner = $SIM_PORTFOLIO_OWNER (must return true)
# A step whose owner variable is unset is reported as skipped.
# Simulate env: SIM_SOURCE  funded G... address used as the tx source
#                           (default: SMOKE_IDENTITY, Friendbot-funded on testnet)
#               HORIZON_URL Horizon for the sequence check (default per network)
set -Eeuo pipefail
trap 'echo "smoke-testnet.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SIMULATE=0
for arg in "$@"; do
  case "$arg" in
    --simulate) SIMULATE=1 ;;
    *) echo "unknown argument: $arg (usage: $0 [--simulate])" >&2; exit 2 ;;
  esac
done

DEPLOYMENT="${DEPLOYMENT:-deployments/testnet.json}"
command -v jq >/dev/null 2>&1 || { echo "missing required command: jq" >&2; exit 1; }
[[ -f "$DEPLOYMENT" ]] || { echo "deployment file not found: $DEPLOYMENT" >&2; exit 1; }
NETWORK="$(jq -r .network "$DEPLOYMENT")"
case "$NETWORK" in
  testnet) ;;
  mainnet) [[ "$SIMULATE" == "1" ]] || { echo "refusing to run against mainnet without --simulate" >&2; exit 1; } ;;
  *) echo "refusing to smoke-test network '$NETWORK'" >&2; exit 1 ;;
esac

for cmd in stellar jq python3; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "missing required command: $cmd" >&2; exit 1; }
done

RPC_URL="$(jq -r .rpc_url "$DEPLOYMENT")"
PASSPHRASE="$(jq -r .network_passphrase "$DEPLOYMENT")"
CONTRACT_ID="$(jq -r .contract_id "$DEPLOYMENT")"
TEMPLATE_ID="$(jq -r '.templates[0].template_id' "$DEPLOYMENT")"
DEPLOY_LEDGER="$(jq -r '.deploy_ledger // "unknown"' "$DEPLOYMENT")"
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

# --- simulate mode -----------------------------------------------------------
if [[ "$SIMULATE" == "1" ]]; then
  if [[ -n "${SIM_SOURCE:-}" ]]; then
    SOURCE="$SIM_SOURCE"
  else
    [[ "$NETWORK" == "testnet" ]] || fail "SIM_SOURCE (a funded account) is required on $NETWORK"
    if ! stellar keys address "$IDENTITY" >/dev/null 2>&1; then
      stellar keys generate "$IDENTITY" "${NET_ARGS[@]}" --fund >/dev/null 2>&1 \
        || fail "could not create/fund identity $IDENTITY via Friendbot"
    fi
    SOURCE="$(stellar keys address "$IDENTITY")"
  fi
  case "$NETWORK" in
    testnet) HORIZON_URL="${HORIZON_URL:-https://horizon-testnet.stellar.org}" ;;
    mainnet) HORIZON_URL="${HORIZON_URL:-https://horizon.stellar.org}" ;;
  esac
  sequence() { curl -sSf "$HORIZON_URL/accounts/$1" | jq -r .sequence; }

  # Simulates one call; prints its return value and stores the resource fee
  # (stroops) in $WORK/fee. Never signs: the source is a public key.
  simulate() {
    local err="$WORK/stderr" out xdr
    if ! out="$(stellar contract invoke --id "$CONTRACT_ID" --source "$SOURCE" "${NET_ARGS[@]}" --send=no -- "$@" 2>"$err")"; then
      cat "$err" >&2
      fail "simulate $* failed"
    fi
    xdr="$(stellar contract invoke --id "$CONTRACT_ID" --source "$SOURCE" "${NET_ARGS[@]}" --build-only -- "$@" \
      | stellar tx simulate --source-account "$SOURCE" "${NET_ARGS[@]}")" || fail "resource simulation of $* failed"
    stellar xdr decode --type TransactionEnvelope --output json <<<"$xdr" \
      | jq -r '.tx.tx.ext.v1.resource_fee // 0' >"$WORK/fee"
    printf '%s' "$out"
  }
  xlm() { awk -v s="$1" 'BEGIN { printf "%.7f", s / 10000000 }'; }

  SIM_STEP=0
  sim_report() {
    local name="$1" owner="$2" result="$3" fee="${4:-}"
    SIM_STEP=$((SIM_STEP + 1))
    local fee_col="-"
    [[ -n "$fee" ]] && fee_col="$(xlm "$fee") XLM"
    if [[ "$MD" == "1" ]]; then
      say "| $SIM_STEP | \`$name\` | ${owner:+\`$owner\`} | $result | $fee_col |"
    else
      say "[$SIM_STEP] $name: $result"
      [[ -n "$owner" ]] && say "    owner: $owner"
      say "    resource fee: $fee_col"
    fi
  }

  SEQ_BEFORE="$(sequence "$SOURCE")" || fail "could not read sequence of $SOURCE from $HORIZON_URL"
  CLAIM_OWNER="$(stellar keys generate "sim-owner-$STAMP" >/dev/null 2>&1; stellar keys address "sim-owner-$STAMP")"
  stellar keys rm "sim-owner-$STAMP" >/dev/null 2>&1 || true
  CID_V1="$(portfolio_cid 1)"
  CID_V2="$(portfolio_cid 2)"

  if [[ "$MD" == "1" ]]; then
    say "- Mode: simulate (\`--send=no\`, nothing submitted)"
    say "- Network: \`$NETWORK\` (\`$RPC_URL\`)"
    say "- Contract: [\`$CONTRACT_ID\`]($EXPLORER/contract/$CONTRACT_ID)"
    say "- Source: \`$SOURCE\` (sequence \`$SEQ_BEFORE\`)"
    say "- Folder: \`$FOLDER\`, template: \`$TEMPLATE_ID\`"
    say ""
    say "| # | Call | Owner | Result | Resource fee |"
    say "|---|---|---|---|---|"
  else
    say "mode:     simulate (nothing is submitted)"
    say "network:  $NETWORK ($RPC_URL)"
    say "contract: $CONTRACT_ID"
    say "source:   $SOURCE (sequence $SEQ_BEFORE)"
  fi

  OUT="$(simulate claim_folder --owner "$CLAIM_OWNER" --folder_name "$FOLDER")"
  [[ "$OUT" == "null" ]] || fail "claim_folder returned $OUT"
  sim_report claim_folder "$CLAIM_OWNER" "ok (read-back skipped)" "$(cat "$WORK/fee")"

  if [[ -n "${SIM_MINT_OWNER:-}" ]]; then
    OUT="$(simulate mint_portfolio --owner "$SIM_MINT_OWNER" --template_id "$TEMPLATE_ID" --ipfs_cid "$CID_V1")"
    [[ "$OUT" == "null" ]] || fail "mint_portfolio returned $OUT"
    sim_report mint_portfolio "$SIM_MINT_OWNER" "ok (read-back skipped)" "$(cat "$WORK/fee")"
  else
    sim_report mint_portfolio "" "skipped (SIM_MINT_OWNER unset)"
  fi

  if [[ -n "${SIM_PORTFOLIO_OWNER:-}" ]]; then
    OUT="$(simulate update_portfolio --owner "$SIM_PORTFOLIO_OWNER" --ipfs_cid "$CID_V2")"
    [[ "$OUT" == "null" ]] || fail "update_portfolio returned $OUT"
    sim_report update_portfolio "$SIM_PORTFOLIO_OWNER" "ok (read-back skipped)" "$(cat "$WORK/fee")"
    OUT="$(simulate verify_portfolio --owner "$SIM_PORTFOLIO_OWNER")"
    [[ "$OUT" == "true" ]] || fail "verify_portfolio returned $OUT"
    sim_report verify_portfolio "$SIM_PORTFOLIO_OWNER" "returned \`true\`" "$(cat "$WORK/fee")"
  else
    sim_report update_portfolio "" "skipped (SIM_PORTFOLIO_OWNER unset)"
    sim_report verify_portfolio "" "skipped (SIM_PORTFOLIO_OWNER unset)"
  fi

  SEQ_AFTER="$(sequence "$SOURCE")" || fail "could not re-read sequence of $SOURCE"
  [[ "$SEQ_AFTER" == "$SEQ_BEFORE" ]] || fail "source sequence changed: $SEQ_BEFORE -> $SEQ_AFTER"
  say ""
  say "OK: simulate run passed; source sequence unchanged ($SEQ_AFTER), 0 transactions submitted"
  exit 0
fi

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
  say "- Deploy ledger: [\`$DEPLOY_LEDGER\`]($EXPLORER/ledger/$DEPLOY_LEDGER)"
  say "- Test account: [\`$USER_ADDR\`]($EXPLORER/account/$USER_ADDR) (fresh, Friendbot-funded)"
  say "- Folder: \`$FOLDER\`, template: \`$TEMPLATE_ID\`"
  say "- Portfolio CIDs: v1 \`$CID_V1\`, v2 \`$CID_V2\`"
  say ""
  say "| # | Call | Result | Transaction |"
  say "|---|---|---|---|"
else
  say "network:  $NETWORK ($RPC_URL)"
  say "contract: $CONTRACT_ID"
  say "deploy ledger: $DEPLOY_LEDGER"
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
