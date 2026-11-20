#!/usr/bin/env bash
# mainnet-preflight.sh — machine-check the mainnet launch conditions.
#
# Usage: ./scripts/mainnet-preflight.sh [deployments/<network>.json]
#
# Read-only: it only simulates contract reads and queries Horizon, RPC, IPFS
# gateways and the GitHub API. It never signs or submits a transaction.
#
# Prints a Markdown PASS/FAIL report and exits 1 if any check fails.
#
# Environment (all optional):
#   DEPLOYMENT       Deployment record (default: deployments/mainnet.json, or $1).
#   HORIZON_URL      Horizon endpoint (default: the network's public Horizon).
#   STELLAR_RPC_URL  RPC endpoint (default: rpc_url from the deployment record).
#   GITHUB_REPO      owner/repo holding the release (default: D-Indexer/Indexer-Contract).
#   RELEASE_TAG      Release tag to compare against (default: the latest release).
#   GITHUB_TOKEN     Optional token for GitHub API rate limits.
#   IPFS_GATEWAYS    Space-separated gateway base URLs
#                    (default: "https://ipfs.io https://dweb.link").
#   MIN_TTL_LEDGERS  Minimum remaining TTL for instance and template entries
#                    (default: 518400, ~30 days).
set -uo pipefail

DEPLOYMENT="${DEPLOYMENT:-${1:-deployments/mainnet.json}}"
GITHUB_REPO="${GITHUB_REPO:-D-Indexer/Indexer-Contract}"
RELEASE_TAG="${RELEASE_TAG:-}"
IPFS_GATEWAYS="${IPFS_GATEWAYS:-https://ipfs.io https://dweb.link}"
MIN_TTL_LEDGERS="${MIN_TTL_LEDGERS:-518400}"

# ── I/O wrappers (overridden by scripts/tests/test-mainnet-preflight.sh) ──────

http_get() {
  local auth=()
  [[ "$1" == https://api.github.com/* && -n "${GITHUB_TOKEN:-}" ]] \
    && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
  curl -sSfL --max-time 30 "${auth[@]}" "$1"
}

# Succeeds if the URL is retrievable (HTTP 2xx after redirects).
http_ok() {
  curl -sfL --max-time 30 -o /dev/null -r 0-0 "$1"
}

rpc_call() {
  local params="${2:-}"
  [[ -n "$params" ]] || params='{}'
  curl -sSf --max-time 30 -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$params}" "$RPC_URL"
}

# Simulates a read-only contract call; never submits.
contract_read() {
  stellar contract invoke --id "$CONTRACT_ID" --source-account "$DEPLOYER" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" --send=no -- "$@"
}

# Prints the base64 LedgerKey XDR for a contract data entry. $1 is an ScVal
# key as JSON, or "instance".
contract_data_key() {
  local key
  if [[ "$1" == "instance" ]]; then
    key='"ledger_key_contract_instance"'
  else
    key="$1"
  fi
  stellar xdr encode --type LedgerKey <<<"{\"contract_data\":{\"contract\":\"$CONTRACT_ID\",\"key\":$key,\"durability\":\"persistent\"}}"
}

# Prints liveUntilLedgerSeq for a LedgerKey XDR, or nothing if it is missing.
live_until() {
  rpc_call getLedgerEntries "{\"keys\":[\"$1\"]}" | jq -r '.result.entries[0].liveUntilLedgerSeq // empty'
}

latest_ledger() {
  rpc_call getLatestLedger | jq -r '.result.sequence'
}

# Prints the sha256 of the wasm the contract currently runs.
onchain_wasm_hash() {
  local tmp
  tmp="$(mktemp)"
  stellar contract fetch --id "$CONTRACT_ID" --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" --out-file "$tmp" >/dev/null 2>&1 \
    && sha256sum "$tmp" | cut -d' ' -f1
  rm -f "$tmp"
}

# ── Report ────────────────────────────────────────────────────────────────────

FAILED=0
REPORT=()
record() { # status name detail
  REPORT+=("| $2 | **$1** | $3 |")
  [[ "$1" == "PASS" ]] || FAILED=1
}

# ── Checks: each prints nothing and calls record once ─────────────────────────

# Admins must be classic accounts whose medium threshold no single signer can
# meet on its own (Soroban account auth uses the medium threshold).
check_admin_multisig() {
  local admins admin acct signers max_weight med bad=()
  if ! admins="$(contract_read get_admins)"; then
    record FAIL "Admins are multisig" "could not read get_admins"
    return
  fi
  for admin in $(jq -r '.[]' <<<"$admins"); do
    if [[ "$admin" != G* ]]; then
      bad+=("$admin: contract account, verify its policy manually")
      continue
    fi
    if ! acct="$(http_get "$HORIZON_URL/accounts/$admin")"; then
      bad+=("$admin: not found on Horizon")
      continue
    fi
    signers="$(jq '[.signers[] | select(.weight > 0)] | length' <<<"$acct")"
    max_weight="$(jq '[.signers[].weight] | max' <<<"$acct")"
    med="$(jq '.thresholds.med_threshold' <<<"$acct")"
    if ((signers < 2 || med <= max_weight)); then
      bad+=("$admin: $signers signer(s), med threshold $med, max signer weight $max_weight")
    fi
  done
  if [[ -z "$(jq -r '.[]' <<<"$admins")" ]]; then
    record FAIL "Admins are multisig" "contract has no admins"
  elif ((${#bad[@]})); then
    record FAIL "Admins are multisig" "single-key admin(s): $(IFS=';'; echo "${bad[*]}")"
  else
    record PASS "Admins are multisig" "$(jq length <<<"$admins") admin(s), each needs 2+ signers"
  fi
}

check_no_admin_is_deployer() {
  local admins
  if ! admins="$(contract_read get_admins)"; then
    record FAIL "No admin is the deployer" "could not read get_admins"
  elif jq -e --arg d "$DEPLOYER" 'index($d) != null' <<<"$admins" >/dev/null; then
    record FAIL "No admin is the deployer" "deployer $DEPLOYER is an admin"
  else
    record PASS "No admin is the deployer" "deployer $DEPLOYER is not an admin"
  fi
}

release_json() {
  if [[ -n "$RELEASE_TAG" ]]; then
    http_get "https://api.github.com/repos/$GITHUB_REPO/releases/tags/$RELEASE_TAG"
  else
    http_get "https://api.github.com/repos/$GITHUB_REPO/releases/latest"
  fi
}

check_wasm_matches_release() {
  local release onchain tag hashes
  if ! release="$(release_json)"; then
    record FAIL "Wasm matches tagged release" "no release found in $GITHUB_REPO${RELEASE_TAG:+ for $RELEASE_TAG}"
    return
  fi
  tag="$(jq -r .tag_name <<<"$release")"
  onchain="$(onchain_wasm_hash)"
  if [[ -z "$onchain" ]]; then
    record FAIL "Wasm matches tagged release" "could not fetch the on-chain wasm"
    return
  fi
  # Matches a .wasm asset digest, or the hash quoted in the release notes.
  hashes="$(jq -r '[.assets[]? | select(.name | endswith(".wasm")) | .digest // empty | sub("^sha256:"; "")] + [(.body // "") | scan("[0-9a-f]{64}")] | .[]' <<<"$release")"
  if grep -qx "$onchain" <<<"$hashes"; then
    record PASS "Wasm matches tagged release" "$onchain matches $tag"
  else
    record FAIL "Wasm matches tagged release" "on-chain $onchain not found in release $tag"
  fi
}

check_template_cids_retrievable() {
  local ids id uri cid gw ok bad=() count=0
  if ! ids="$(contract_read list_templates)"; then
    record FAIL "Template CIDs retrievable" "could not read list_templates"
    return
  fi
  for id in $(jq -r '.[]' <<<"$ids"); do
    count=$((count + 1))
    uri="$(contract_read get_template --template_id "$id" | jq -r .metadata_uri)"
    cid="${uri#ipfs://}"
    ok=0
    for gw in $IPFS_GATEWAYS; do
      if http_ok "$gw/ipfs/$cid"; then ok=1; break; fi
    done
    ((ok)) || bad+=("template $id ($cid)")
  done
  if ((${#bad[@]})); then
    record FAIL "Template CIDs retrievable" "not retrievable from any gateway: $(IFS=','; echo "${bad[*]}")"
  else
    record PASS "Template CIDs retrievable" "$count template(s) retrievable"
  fi
}

check_ttls() {
  local latest ids id key until remaining bad=()
  if ! latest="$(latest_ledger)" || [[ -z "$latest" ]]; then
    record FAIL "Instance and template TTLs" "could not read the latest ledger"
    return
  fi
  ttl_of() { # label ScVal-json
    key="$(contract_data_key "$2")"
    until="$(live_until "$key")"
    if [[ -z "$until" ]]; then
      bad+=("$1: missing or archived")
      return
    fi
    remaining=$((until - latest))
    ((remaining >= MIN_TTL_LEDGERS)) || bad+=("$1: $remaining ledgers left")
  }
  ttl_of instance instance
  ids="$(contract_read list_templates || echo '[]')"
  for id in $(jq -r '.[]' <<<"$ids"); do
    ttl_of "template $id" "{\"vec\":[{\"symbol\":\"Template\"},{\"u32\":$id}]}"
  done
  ttl_of "template list" '{"vec":[{"symbol":"TemplateList"}]}'
  if ((${#bad[@]})); then
    record FAIL "Instance and template TTLs" "below $MIN_TTL_LEDGERS ledgers: $(IFS=';'; echo "${bad[*]}")"
  else
    record PASS "Instance and template TTLs" "all at least $MIN_TTL_LEDGERS ledgers"
  fi
}

check_ci_green() {
  local release ref sha runs total pending failed
  if [[ -n "$RELEASE_TAG" ]]; then
    ref="$RELEASE_TAG"
  elif release="$(release_json)"; then
    ref="$(jq -r .tag_name <<<"$release")"
  else
    ref="$(jq -r .source_commit "$DEPLOYMENT")"
  fi
  if ! sha="$(http_get "https://api.github.com/repos/$GITHUB_REPO/commits/$ref" | jq -r .sha)" || [[ -z "$sha" || "$sha" == null ]]; then
    record FAIL "CI green on release commit" "could not resolve $ref"
    return
  fi
  if ! runs="$(http_get "https://api.github.com/repos/$GITHUB_REPO/commits/$sha/check-runs?per_page=100")"; then
    record FAIL "CI green on release commit" "could not read check runs for $sha"
    return
  fi
  total="$(jq '.check_runs | length' <<<"$runs")"
  pending="$(jq '[.check_runs[] | select(.status != "completed")] | length' <<<"$runs")"
  failed="$(jq -r '[.check_runs[] | select(.status == "completed" and (.conclusion | IN("success", "skipped", "neutral") | not)) | .name] | join(", ")' <<<"$runs")"
  if ((total == 0)); then
    record FAIL "CI green on release commit" "no check runs on $sha"
  elif ((pending > 0)); then
    record FAIL "CI green on release commit" "$pending check run(s) still running on $sha"
  elif [[ -n "$failed" ]]; then
    record FAIL "CI green on release commit" "failed on $sha: $failed"
  else
    record PASS "CI green on release commit" "$total check run(s) green on $sha"
  fi
}

load_deployment() {
  [[ -f "$DEPLOYMENT" ]] || { echo "mainnet-preflight.sh: deployment record $DEPLOYMENT not found" >&2; exit 2; }
  NETWORK="$(jq -r .network "$DEPLOYMENT")"
  CONTRACT_ID="$(jq -r .contract_id "$DEPLOYMENT")"
  DEPLOYER="$(jq -r .deployer "$DEPLOYMENT")"
  PASSPHRASE="$(jq -r .network_passphrase "$DEPLOYMENT")"
  RPC_URL="${STELLAR_RPC_URL:-$(jq -r .rpc_url "$DEPLOYMENT")}"
  case "$NETWORK" in
    mainnet) HORIZON_URL="${HORIZON_URL:-https://horizon.stellar.org}" ;;
    testnet) HORIZON_URL="${HORIZON_URL:-https://horizon-testnet.stellar.org}" ;;
    *) HORIZON_URL="${HORIZON_URL:-http://localhost:8000}" ;;
  esac
}

print_report() {
  echo "## Mainnet preflight: $NETWORK \`$CONTRACT_ID\`"
  echo
  echo "| Check | Result | Detail |"
  echo "|---|---|---|"
  printf '%s\n' "${REPORT[@]}"
  echo
  if ((FAILED)); then echo "**Overall: FAIL**"; else echo "**Overall: PASS**"; fi
}

main() {
  for cmd in stellar jq curl sha256sum; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "mainnet-preflight.sh: missing required command: $cmd" >&2; exit 2; }
  done
  load_deployment
  check_admin_multisig
  check_no_admin_is_deployer
  check_wasm_matches_release
  check_template_cids_retrievable
  check_ttls
  check_ci_green
  print_report
  return "$FAILED"
}

# Sourcing with PREFLIGHT_LIB=1 loads the functions without running checks.
if [[ "${PREFLIGHT_LIB:-0}" != "1" ]]; then
  main
fi
