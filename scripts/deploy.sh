#!/usr/bin/env bash
# deploy.sh — build, deploy, initialize and seed the Folder contract.
#
# Usage: ./scripts/deploy.sh [testnet|mainnet]
#
# Environment (all optional):
#   STELLAR_IDENTITY   Stellar CLI identity that pays for and signs the deploy
#                      (default: folder-deployer). Created and funded through
#                      Friendbot on testnet if it does not exist.
#   ADMIN_IDENTITY     Identity that becomes the first admin and registers the
#                      templates (default: $STELLAR_IDENTITY). It must be a
#                      local identity because `initialize` requires its signature.
#   STELLAR_RPC_URL    RPC endpoint (default: the network's public RPC).
#   TEMPLATES_MANIFEST Template manifest to register (default: templates/manifest.json).
#                      Set to an empty string to skip template registration.
#
# Writes deployments/<network>.json. Exits non-zero on the first failure.
set -Eeuo pipefail
trap 'echo "deploy.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

NETWORK="${1:-testnet}"
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
MANIFEST="${TEMPLATES_MANIFEST-templates/manifest.json}"
WASM="target/stellar/indexer_contract.wasm"
OUT="deployments/$NETWORK.json"
NET_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

die() { echo "deploy.sh: $*" >&2; exit 1; }
for cmd in stellar cargo make jq python3 sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

# Runs a stellar CLI command, echoes stdout, and stores the tx hash (if any)
# in LAST_TX. stderr is kept so failures are shown in full.
LAST_TX=""
run_stellar() {
  local err="$LOG_DIR/stderr"
  local out
  if ! out="$(stellar "$@" 2>"$err")"; then
    cat "$err" >&2
    die "stellar $1 $2 failed"
  fi
  LAST_TX="$(grep -oE 'tx/[0-9a-f]{64}' "$err" | tail -1 | cut -d/ -f2 || true)"
  printf '%s' "$out"
}

ensure_identity() {
  local name="$1"
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    [[ "$NETWORK" == "testnet" ]] || die "identity '$name' not found; create it with 'stellar keys add $name'"
    echo "==> Creating identity '$name' and funding it via Friendbot"
    stellar keys generate "$name" "${NET_ARGS[@]}" --fund >/dev/null
  elif [[ "$NETWORK" == "testnet" ]]; then
    echo "==> Ensuring '$name' is funded via Friendbot"
    stellar keys fund "$name" "${NET_ARGS[@]}" >/dev/null 2>&1 || true
  fi
  stellar keys address "$name" >/dev/null || die "identity '$name' unusable"
}

ensure_identity "$IDENTITY"
[[ "$ADMIN_IDENTITY" == "$IDENTITY" ]] || ensure_identity "$ADMIN_IDENTITY"
DEPLOYER_ADDRESS="$(stellar keys address "$IDENTITY")"
ADMIN_ADDRESS="$(stellar keys address "$ADMIN_IDENTITY")"

echo "==> Building optimized WASM"
make -s build-optimized >/dev/null
LOCAL_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
echo "    local wasm sha256: $LOCAL_HASH"

if [[ -n "$MANIFEST" ]]; then
  echo "==> Verifying template CIDs match committed files"
  python3 -I scripts/template_cids.py --check
fi

echo "==> Uploading WASM to $NETWORK"
WASM_HASH="$(run_stellar contract upload --wasm "$WASM" --source "$IDENTITY" "${NET_ARGS[@]}")"
[[ "$WASM_HASH" == "$LOCAL_HASH" ]] || die "uploaded wasm hash $WASM_HASH != local $LOCAL_HASH"
UPLOAD_TX="$LAST_TX"

echo "==> Deploying contract"
CONTRACT_ID="$(run_stellar contract deploy --wasm-hash "$WASM_HASH" --source "$IDENTITY" "${NET_ARGS[@]}")"
[[ "$CONTRACT_ID" =~ ^C[A-Z2-7]{55}$ ]] || die "unexpected contract id: $CONTRACT_ID"
DEPLOY_TX="$LAST_TX"
echo "    contract id: $CONTRACT_ID"

invoke() {
  run_stellar contract invoke --id "$CONTRACT_ID" --source "$ADMIN_IDENTITY" "${NET_ARGS[@]}" -- "$@"
}

echo "==> Initializing with admin $ADMIN_ADDRESS"
# initialize is a separate transaction from deploy. If anyone front-runs it,
# this call fails with AlreadyInitialized (#1) and the admin check below fails.
invoke initialize --admin "$ADMIN_ADDRESS" >/dev/null
INIT_TX="$LAST_TX"
ADMINS="$(invoke get_admins)"
[[ "$(jq -c . <<<"$ADMINS")" == "$(jq -cn --arg a "$ADMIN_ADDRESS" '[$a]')" ]] \
  || die "admin check failed: expected [$ADMIN_ADDRESS], got $ADMINS — do not use this contract"

TEMPLATES_JSON="[]"
if [[ -n "$MANIFEST" ]]; then
  echo "==> Registering templates from $MANIFEST"
  while IFS=$'\t' read -r id uri; do
    invoke register_template --admin "$ADMIN_ADDRESS" --template_id "$id" --metadata_uri "$uri" >/dev/null
    echo "    template $id -> $uri (tx $LAST_TX)"
    TEMPLATES_JSON="$(jq -c --argjson id "$id" --arg uri "$uri" --arg tx "$LAST_TX" \
      '. + [{template_id: $id, metadata_uri: $uri, tx_hash: $tx}]' <<<"$TEMPLATES_JSON")"
  done < <(jq -r '.[] | [.template_id, .metadata_uri] | @tsv' "$MANIFEST")
  ON_CHAIN="$(invoke list_templates)"
  [[ "$(jq -c 'sort' <<<"$ON_CHAIN")" == "$(jq -c '[.[].template_id] | sort' "$MANIFEST")" ]] \
    || die "list_templates returned $ON_CHAIN, expected ids from $MANIFEST"
fi

mkdir -p deployments
jq -n \
  --arg network "$NETWORK" \
  --arg rpc_url "$RPC_URL" \
  --arg passphrase "$PASSPHRASE" \
  --arg contract_id "$CONTRACT_ID" \
  --arg wasm_hash "$WASM_HASH" \
  --arg admin "$ADMIN_ADDRESS" \
  --arg deployer "$DEPLOYER_ADDRESS" \
  --arg deployed_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg commit "$(git rev-parse HEAD)" \
  --arg cli "$(stellar --version | head -1)" \
  --arg rustc "$(rustc --version)" \
  --arg upload_tx "$UPLOAD_TX" \
  --arg deploy_tx "$DEPLOY_TX" \
  --arg init_tx "$INIT_TX" \
  --argjson templates "$TEMPLATES_JSON" \
  '{
    network: $network,
    rpc_url: $rpc_url,
    network_passphrase: $passphrase,
    contract_id: $contract_id,
    wasm_hash: $wasm_hash,
    admin: $admin,
    deployer: $deployer,
    deployed_at: $deployed_at,
    source_commit: $commit,
    toolchain: {stellar_cli: $cli, rustc: $rustc},
    explorer: ("https://stellar.expert/explorer/" + $network + "/contract/" + $contract_id),
    transactions: {upload_wasm: $upload_tx, deploy: $deploy_tx, initialize: $init_tx},
    templates: $templates
  }' >"$OUT"

echo "==> Done. Wrote $OUT"
echo "CONTRACT_ID=$CONTRACT_ID"
echo "WASM_HASH=$WASM_HASH"
