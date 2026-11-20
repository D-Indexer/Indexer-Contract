#!/usr/bin/env bash
# Per-check tests for scripts/mainnet-preflight.sh with mocked Horizon, RPC,
# contract reads and GitHub responses. No network access.
# Usage: ./scripts/tests/test-mainnet-preflight.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PREFLIGHT_LIB=1 source "$ROOT/scripts/mainnet-preflight.sh"

ADMIN_A=GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF
ADMIN_B=GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
DEPLOYER=GDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD
CONTRACT_ID=CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
HORIZON_URL=https://horizon.test
GITHUB_REPO=org/repo
RELEASE_TAG=v1.0.0
IPFS_GATEWAYS="https://gw1 https://gw2"
MIN_TTL_LEDGERS=100
HASH=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa

# ── Mocks: each reads its fixture from a MOCK_* variable ──────────────────────
MOCK_ADMINS="[\"$ADMIN_A\"]"
declare -A MOCK_HTTP MOCK_LIVE_UNTIL
MOCK_OK_URLS=""
MOCK_LATEST=1000
MOCK_WASM=$HASH

contract_read() {
  case "$1" in
    get_admins) [[ "$MOCK_ADMINS" != ERR ]] && echo "$MOCK_ADMINS" ;;
    list_templates) echo '[1,2]' ;;
    get_template) echo "{\"metadata_uri\":\"ipfs://cid$3\"}" ;;
  esac
}
http_get() { [[ -n "${MOCK_HTTP[$1]:-}" ]] && echo "${MOCK_HTTP[$1]}"; }
http_ok() { [[ " $MOCK_OK_URLS " == *" $1 "* ]]; }
contract_data_key() { echo "$1"; }
live_until() { echo "${MOCK_LIVE_UNTIL[$1]:-}"; }
latest_ledger() { echo "$MOCK_LATEST"; }
onchain_wasm_hash() { echo "$MOCK_WASM"; }

account() { # signer weights..., then med threshold as last arg
  local med="${*: -1}" weights=("${@:1:$#-1}") s=""
  for w in "${weights[@]}"; do s+="{\"weight\":$w},"; done
  echo "{\"signers\":[${s%,}],\"thresholds\":{\"low_threshold\":0,\"med_threshold\":$med,\"high_threshold\":$med}}"
}

PASSES=0 FAILS=0
# expect <PASS|FAIL> <detail substring> <check function>
expect() {
  REPORT=() FAILED=0
  "$3"
  local line="${REPORT[0]:-}"
  if [[ "$line" == *"**$1**"* && "$line" == *"$2"* ]]; then
    PASSES=$((PASSES + 1))
  else
    FAILS=$((FAILS + 1))
    echo "not ok: $3 expected $1 containing '$2', got: $line" >&2
  fi
}

# ── Admin multisig ────────────────────────────────────────────────────────────
MOCK_HTTP["$HORIZON_URL/accounts/$ADMIN_A"]="$(account 1 0)"
expect FAIL "1 signer(s)" check_admin_multisig            # testnet: single key
MOCK_HTTP["$HORIZON_URL/accounts/$ADMIN_A"]="$(account 1 1 1 2)"
expect PASS "1 admin(s)" check_admin_multisig            # 2-of-3
MOCK_HTTP["$HORIZON_URL/accounts/$ADMIN_A"]="$(account 2 1 2)"
expect FAIL "max signer weight 2" check_admin_multisig    # one key meets threshold
MOCK_ADMINS="[\"$ADMIN_A\",\"$ADMIN_B\"]"
expect FAIL "$ADMIN_B: not found" check_admin_multisig    # missing account
MOCK_ADMINS='["CCONTRACTADMIN"]'
expect FAIL "contract account" check_admin_multisig
MOCK_ADMINS=ERR
expect FAIL "could not read" check_admin_multisig

# ── Deployer is not an admin ──────────────────────────────────────────────────
MOCK_ADMINS="[\"$ADMIN_A\",\"$DEPLOYER\"]"
expect FAIL "is an admin" check_no_admin_is_deployer
MOCK_ADMINS="[\"$ADMIN_A\"]"
expect PASS "is not an admin" check_no_admin_is_deployer

# ── Wasm matches release ──────────────────────────────────────────────────────
REL="https://api.github.com/repos/$GITHUB_REPO/releases/tags/$RELEASE_TAG"
expect FAIL "no release found" check_wasm_matches_release
MOCK_HTTP[$REL]="{\"tag_name\":\"v1.0.0\",\"assets\":[{\"name\":\"indexer_contract.wasm\",\"digest\":\"sha256:$HASH\"}]}"
expect PASS "matches v1.0.0" check_wasm_matches_release
MOCK_HTTP[$REL]="{\"tag_name\":\"v1.0.0\",\"assets\":[],\"body\":\"wasm hash: \`$HASH\`\"}"
expect PASS "matches v1.0.0" check_wasm_matches_release
MOCK_WASM=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
expect FAIL "not found in release" check_wasm_matches_release
MOCK_WASM=""
expect FAIL "could not fetch" check_wasm_matches_release
MOCK_WASM=$HASH

# ── Template CIDs ─────────────────────────────────────────────────────────────
MOCK_OK_URLS="https://gw2/ipfs/cid1"
expect FAIL "template 2 (cid2)" check_template_cids_retrievable
MOCK_OK_URLS="https://gw2/ipfs/cid1 https://gw1/ipfs/cid2"
expect PASS "2 template(s)" check_template_cids_retrievable

# ── TTLs ──────────────────────────────────────────────────────────────────────
T1='{"vec":[{"symbol":"Template"},{"u32":1}]}'
T2='{"vec":[{"symbol":"Template"},{"u32":2}]}'
TL='{"vec":[{"symbol":"TemplateList"}]}'
MOCK_LIVE_UNTIL=([instance]=2000 [$T1]=2000 [$T2]=2000 [$TL]=2000)
expect PASS "at least 100" check_ttls
MOCK_LIVE_UNTIL[$T2]=1099
expect FAIL "template 2: 99 ledgers left" check_ttls
MOCK_LIVE_UNTIL[$T2]=1100
unset 'MOCK_LIVE_UNTIL[instance]'
expect FAIL "instance: missing" check_ttls

# ── CI green ──────────────────────────────────────────────────────────────────
SHA=0123456789abcdef0123456789abcdef01234567
MOCK_HTTP["https://api.github.com/repos/$GITHUB_REPO/commits/$RELEASE_TAG"]="{\"sha\":\"$SHA\"}"
RUNS="https://api.github.com/repos/$GITHUB_REPO/commits/$SHA/check-runs?per_page=100"
MOCK_HTTP[$RUNS]='{"check_runs":[]}'
expect FAIL "no check runs" check_ci_green
MOCK_HTTP[$RUNS]='{"check_runs":[{"name":"test","status":"completed","conclusion":"success"},{"name":"smoke","status":"completed","conclusion":"skipped"}]}'
expect PASS "2 check run(s) green" check_ci_green
MOCK_HTTP[$RUNS]='{"check_runs":[{"name":"test","status":"completed","conclusion":"failure"}]}'
expect FAIL "failed on $SHA: test" check_ci_green
MOCK_HTTP[$RUNS]='{"check_runs":[{"name":"test","status":"in_progress","conclusion":null}]}'
expect FAIL "still running" check_ci_green

echo "mainnet-preflight: $PASSES passed, $FAILS failed"
((FAILS == 0))
