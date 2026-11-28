#!/usr/bin/env bash
# multisig-admin-demo.sh — 2-of-3 classic multisig admin flow on testnet.
#
# Creates three Friendbot-funded identities, turns the first one into a 2-of-3
# classic multisig account (set_options: master weight 1, signers A/B weight 1,
# low/med/high thresholds 2), deploys a throwaway contract, makes the multisig
# account its admin, then registers a template with a transaction signed by two
# of the three signers. A one-signature attempt is expected to fail and is
# reported as such. Every submitted transaction hash is printed.
#
# Usage: ./scripts/multisig-admin-demo.sh
# Env:   MULTISIG_MARKDOWN=1 prints a Markdown report instead of plain text.
#        MULTISIG_KEEP=1 keeps the temporary working directory.
set -Eeuo pipefail
trap 'echo "multisig-admin-demo.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"
PASSPHRASE="Test SDF Network ; September 2015"
NET_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
EXPLORER="https://stellar.expert/explorer/$NETWORK"
WASM="target/stellar/indexer_contract.wasm"
MD="${MULTISIG_MARKDOWN:-0}"

for cmd in stellar jq make sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "missing required command: $cmd" >&2; exit 1; }
done

WORK="$(mktemp -d)"
if [[ "${MULTISIG_KEEP:-0}" == "1" ]]; then
  echo "work dir kept: $WORK"
else
  trap 'rm -rf "$WORK"' EXIT
fi

fail() { echo "multisig-admin-demo.sh: $*" >&2; exit 1; }
say() { echo "$@"; }

# Command substitution runs in a subshell, so the last tx hash is passed
# through a file rather than a variable.
last_tx() { cat "$WORK/last_tx" 2>/dev/null || true; }
run_stellar() {
  local err="$WORK/stderr" out
  if ! out="$(stellar "$@" 2>"$err")"; then
    cat "$err" >&2
    fail "stellar $1 $2 failed"
  fi
  { grep -oE '(Signing transaction: |tx/)[0-9a-f]{64}' "$err" | tail -1 | grep -oE '[0-9a-f]{64}' || true; } >"$WORK/last_tx"
  printf '%s' "$out"
}

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

STAMP="$(date -u +%s)"
ensure_identity() {
  local name="$1"
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    stellar keys generate "$name" "${NET_ARGS[@]}" --fund >/dev/null 2>&1 \
      || fail "could not create/fund identity $name via Friendbot"
  else
    stellar keys fund "$name" "${NET_ARGS[@]}" >/dev/null 2>&1 || true
  fi
  stellar keys address "$name" >/dev/null || fail "identity $name unusable"
}

# 1. Three signer identities.
MASTER="ms-demo-master-$STAMP"
SIGNER_A="ms-demo-a-$STAMP"
SIGNER_B="ms-demo-b-$STAMP"
for id in "$MASTER" "$SIGNER_A" "$SIGNER_B"; do ensure_identity "$id"; done
MASTER_ADDR="$(stellar keys address "$MASTER")"
A_ADDR="$(stellar keys address "$SIGNER_A")"
B_ADDR="$(stellar keys address "$SIGNER_B")"

if [[ "$MD" == "1" ]]; then
  say "- Network: \`$NETWORK\` (\`$RPC_URL\`)"
  say "- Multisig account (2-of-3): [\`$MASTER_ADDR\`]($EXPLORER/account/$MASTER_ADDR)"
  say "- Signers: [\`$A_ADDR\`]($EXPLORER/account/$A_ADDR), [\`$B_ADDR\`]($EXPLORER/account/$B_ADDR)"
  say ""
  say "| # | Step | Result | Transaction |"
  say "|---|---|---|---|"
else
  say "network:  $NETWORK ($RPC_URL)"
  say "account:  $MASTER_ADDR (2-of-3)"
  say "signers:  $A_ADDR, $B_ADDR"
fi

# 2. set_options: master weight 1, two signers weight 1, thresholds 2.
# The master key alone can no longer reach the medium threshold, so every
# later transaction needs two signatures.
run_stellar keys fund "$MASTER" "${NET_ARGS[@]}" >/dev/null 2>&1 || true
run_stellar tx new set-options \
  --source-account "$MASTER" \
  --signer "ed25519:$A_ADDR:1" \
  --signer "ed25519:$B_ADDR:1" \
  --master-weight 1 \
  --low-threshold 2 \
  --medium-threshold 2 \
  --high-threshold 2 \
  "${NET_ARGS[@]}" >/dev/null
report set_options "$(last_tx)" "2-of-3 signers installed, thresholds = 2"

# 3. Throwaway deployment, initialized with the multisig account as admin.
say "==> Building optimized WASM"
make -s build-optimized >/dev/null
LOCAL_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
WASM_HASH="$(run_stellar contract upload --wasm "$WASM" --source "$MASTER" "${NET_ARGS[@]}")"
[[ "$WASM_HASH" == "$LOCAL_HASH" ]] || fail "uploaded wasm hash $WASM_HASH != local $LOCAL_HASH"
report upload_wasm "$(last_tx)" "wasm hash $WASM_HASH"

CONTRACT_ID="$(run_stellar contract deploy --wasm-hash "$WASM_HASH" --source "$MASTER" "${NET_ARGS[@]}")"
[[ "$CONTRACT_ID" =~ ^C[A-Z2-7]{55}$ ]] || fail "unexpected contract id: $CONTRACT_ID"
report deploy_contract "$(last_tx)" "contract \`$CONTRACT_ID\`"

run_stellar contract invoke --id "$CONTRACT_ID" --source "$MASTER" "${NET_ARGS[@]}" \
  -- initialize --admin "$MASTER_ADDR" >/dev/null
report initialize "$(last_tx)" "admin = multisig account"

ADMINS="$(run_stellar contract invoke --id "$CONTRACT_ID" --source "$MASTER" "${NET_ARGS[@]}" \
  --send=no -- get_admins)"
[[ "$(jq -c . <<<"$ADMINS")" == "$(jq -cn --arg a "$MASTER_ADDR" '[$a]')" ]] \
  || fail "get_admins returned $ADMINS, expected [$MASTER_ADDR]"

# 4. Build register_template without submitting, then sign with two identities.
TEMPLATE_ID=$((STAMP % 100000 + 1))
METADATA_URI="ipfs://bafkreie33ewzcjvpo3gldpvqgevtoy4oaygkqri3cwxqr6o6lsqc4jqvwu"
XDR="$WORK/register_template.xdr"
run_stellar contract invoke --id "$CONTRACT_ID" --source "$MASTER" "${NET_ARGS[@]}" \
  --build-only -- register_template \
  --admin "$MASTER_ADDR" --template_id "$TEMPLATE_ID" --metadata_uri "$METADATA_URI" >"$XDR"
[[ -s "$XDR" ]] || fail "register_template --build-only produced no envelope"

# 4a. One signature must not be enough: sign with A only and expect failure.
ONE_SIG="$WORK/one-sig.xdr"
run_stellar tx sign "$XDR" --sign-with-key "$SIGNER_A" "${NET_ARGS[@]}" >"$ONE_SIG"
if run_stellar tx send "$ONE_SIG" "${NET_ARGS[@]}" >/dev/null 2>&1; then
  fail "one-signature register_template unexpectedly succeeded"
fi
ONE_SIG_TX="$(last_tx)"
if [[ "$MD" == "1" ]]; then
  STEP=$((STEP + 1))
  say "| $STEP | \`register_template (1 signature)\` | rejected as expected (txError: tx_bad_auth) | [\`${ONE_SIG_TX:-n/a}\`]($EXPLORER/tx/${ONE_SIG_TX:-n/a}) |"
else
  STEP=$((STEP + 1))
  say "[$STEP] register_template with one signature: rejected as expected"
  say "    tx: ${ONE_SIG_TX:-n/a}"
fi

# 4b. Two signatures: A and B.
TWO_SIG="$WORK/two-sig.xdr"
run_stellar tx sign "$XDR" --sign-with-key "$SIGNER_A" "${NET_ARGS[@]}" >"$WORK/sig-a.xdr"
run_stellar tx sign "$WORK/sig-a.xdr" --sign-with-key "$SIGNER_B" "${NET_ARGS[@]}" >"$TWO_SIG"
run_stellar tx send "$TWO_SIG" "${NET_ARGS[@]}" >/dev/null
report register_template "$(last_tx)" "signed by 2 of 3 signers, template_id = $TEMPLATE_ID"

# 5. Verify on-chain state.
TEMPLATE="$(run_stellar contract invoke --id "$CONTRACT_ID" --source "$MASTER" "${NET_ARGS[@]}" \
  --send=no -- get_template --template_id "$TEMPLATE_ID")"
[[ "$(jq -r .metadata_uri <<<"$TEMPLATE")" == "$METADATA_URI" ]] || fail "get_template mismatch: $TEMPLATE"
say "verified: get_template($TEMPLATE_ID).metadata_uri = $METADATA_URI"
say "done: 2-of-3 multisig admin flow completed on $NETWORK"
