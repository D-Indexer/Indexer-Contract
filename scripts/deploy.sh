#!/usr/bin/env bash
# deploy.sh — build, deploy, initialize and seed the Folder contract.
#
# Usage: ./scripts/deploy.sh [testnet|mainnet] [--resume|--fresh] [--build-only|--submit FILE]
#
#   --resume        Continue the run saved in deploy-out/<network>/state.json:
#                   completed steps are skipped and only templates missing from
#                   list_templates() are registered, on the same contract.
#   --fresh         Discard the saved state and deploy a new contract.
#   --build-only    Sign nothing. Write the unsigned XDR of the next pending step
#                   to deploy-out/<network>/<step>.xdr, with a summary for signers
#                   in <step>.txt, then exit.
#   --submit FILE   Submit FILE, the signed XDR of the pending step, verify the
#                   result and record it.
#
# Offline or multisig signing runs one step at a time (--build-only, sign,
# --submit, repeat) so each transaction is built on the current sequence number:
#   upload_wasm, deploy, initialize, template-<id> for each manifest entry.
#
# Environment (all optional):
#   STELLAR_IDENTITY   Stellar CLI identity that pays for and signs the deploy
#                      (default: folder-deployer). Created and funded through
#                      Friendbot on testnet if it does not exist.
#   ADMIN_IDENTITY     Identity that becomes the first admin and registers the
#                      templates (default: $STELLAR_IDENTITY). It must be a
#                      local identity because `initialize` requires its signature.
#   ADMIN_ADDRESS      Required with --build-only/--submit instead of the two
#                      identities: the G... admin account, which is also the
#                      source account of every transaction. No key is needed.
#   STELLAR_RPC_URL    RPC endpoint (default: the network's public RPC).
#   TEMPLATES_MANIFEST Template manifest to register (default: templates/manifest.json).
#                      Set to an empty string to skip template registration.
#
# Writes deployments/<network>.json once every step is done, after validating
# it against deployments/schema.json. Exits non-zero on the first failure.
# jq filters below use $variables in single quotes on purpose.
# shellcheck disable=SC2016
set -Eeuo pipefail
trap 'echo "deploy.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

die() { echo "deploy.sh: $*" >&2; exit 1; }

NETWORK="testnet"
MODE="local"
STATE_MODE=""
SIGNED_XDR=""
while (($#)); do
  case "$1" in
    testnet | mainnet) NETWORK="$1" ;;
    --resume | --fresh) STATE_MODE="${1#--}" ;;
    --build-only) MODE="build" ;;
    --submit)
      MODE="submit"
      [[ -f "${2:-}" ]] || die "--submit needs the signed XDR file"
      SIGNED_XDR="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
      shift
      ;;
    *) die "Unsupported argument: $1. Expected 'testnet' or 'mainnet' and the options in the header." ;;
  esac
  shift
done

case "$NETWORK" in
  testnet)
    DEFAULT_RPC_URL="https://soroban-testnet.stellar.org"
    PASSPHRASE="Test SDF Network ; September 2015"
    ;;
  mainnet)
    DEFAULT_RPC_URL="https://mainnet.sorobanrpc.com"
    PASSPHRASE="Public Global Stellar Network ; September 2015"
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
STATE_DIR="deploy-out/$NETWORK"
STATE="$STATE_DIR/state.json"
NET_ARGS=(--network "$NETWORK" --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

for cmd in stellar cargo make jq python3 sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

# --- state file ------------------------------------------------------------
[[ "$STATE_MODE" != "fresh" ]] || rm -rf "$STATE_DIR"
if [[ -f "$STATE" ]]; then
  # --build-only/--submit are step-wise by design and always continue.
  [[ -n "$STATE_MODE" || "$MODE" != "local" ]] \
    || die "$STATE exists from an earlier run; pass --resume to continue it or --fresh to start over"
else
  [[ "$MODE" != "submit" ]] || die "no $STATE: run --build-only first"
  mkdir -p "$STATE_DIR"
  echo '{"steps": {}}' >"$STATE"
fi
state() { jq -r "$1 // empty" "$STATE"; }
state_set() {
  local filter="$1"
  shift
  jq "$@" "$filter" "$STATE" >"$STATE.tmp" && mv "$STATE.tmp" "$STATE"
}
step_done() { jq -e --arg s "$1" '.steps[$s]' "$STATE" >/dev/null; }
tx_ledger() { python3 -I scripts/backfill_ledgers.py ledger "$1" --rpc-url "$RPC_URL" --network "$NETWORK"; }
# record_step STEP TX — marks STEP done with its tx hash and ledger.
record_step() {
  local ledger="null"
  if [[ "$2" =~ ^[0-9a-f]{64}$ ]]; then ledger="$(tx_ledger "$2")"; fi
  state_set '.steps[$s] = {tx: $tx, ledger: $l} | del(.pending)' \
    --arg s "$1" --arg tx "$2" --argjson l "$ledger"
}

# Runs a stellar CLI command, echoes stdout, and stores the tx hash (if any)
# for last_tx. stderr is kept so failures are shown in full.
# Command substitution runs in a subshell, so the last tx hash is passed
# through a file rather than a variable.
last_tx() { cat "$LOG_DIR/last_tx" 2>/dev/null || true; }
run_stellar() {
  local err="$LOG_DIR/stderr"
  local out
  if ! out="$(stellar "$@" 2>"$err")"; then
    cat "$err" >&2
    die "stellar $1 $2 failed"
  fi
  # The CLI logs "Signing transaction: <hash>" for every submitted tx. A wasm
  # upload that is already on-chain submits nothing, leaving last_tx empty.
  { grep -oE '(Signing transaction: |tx/)[0-9a-f]{64}' "$err" | tail -1 | grep -oE '[0-9a-f]{64}' || true; } >"$LOG_DIR/last_tx"
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

if [[ "$MODE" == "local" ]]; then
  ensure_identity "$IDENTITY"
  [[ "$ADMIN_IDENTITY" == "$IDENTITY" ]] || ensure_identity "$ADMIN_IDENTITY"
  DEPLOYER_ADDRESS="$(stellar keys address "$IDENTITY")"
  ADMIN_ADDRESS="$(stellar keys address "$ADMIN_IDENTITY")"
  SOURCE="$IDENTITY"
  ADMIN_SOURCE="$ADMIN_IDENTITY"
else
  [[ "${ADMIN_ADDRESS:-}" =~ ^G[A-Z2-7]{55}$ ]] \
    || die "set ADMIN_ADDRESS to the signing G... account for --build-only/--submit"
  DEPLOYER_ADDRESS="$ADMIN_ADDRESS"
  SOURCE="$ADMIN_ADDRESS"
  ADMIN_SOURCE="$ADMIN_ADDRESS"
fi

echo "==> Building optimized WASM"
make -s build-optimized >/dev/null
LOCAL_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
echo "    local wasm sha256: $LOCAL_HASH"
SAVED_HASH="$(state .wasm_hash)"
[[ -z "$SAVED_HASH" || "$SAVED_HASH" == "$LOCAL_HASH" ]] \
  || die "local wasm $LOCAL_HASH differs from the saved run's $SAVED_HASH; use --fresh"
state_set '.wasm_hash = $h' --arg h "$LOCAL_HASH"
WASM_HASH="$LOCAL_HASH"

if [[ -n "$MANIFEST" ]]; then
  echo "==> Verifying template CIDs match committed files"
  python3 -I scripts/template_cids.py --check
fi

CONTRACT_ID="$(state .contract_id)"
invoke() {
  run_stellar contract invoke --id "$CONTRACT_ID" --source "$ADMIN_SOURCE" "${NET_ARGS[@]}" -- "$@"
}

# --- verification shared by local and --submit runs --------------------------
verify_fetched_wasm() { # stellar contract fetch selector...
  local file="$LOG_DIR/fetched.wasm"
  run_stellar contract fetch "$@" "${NET_ARGS[@]}" --out-file "$file" >/dev/null
  [[ "$(sha256sum "$file" | cut -d' ' -f1)" == "$WASM_HASH" ]] || die "on-chain wasm for $* != $WASM_HASH"
}
verify_admins() {
  local admins
  admins="$(invoke get_admins)"
  [[ "$(jq -c . <<<"$admins")" == "$(jq -cn --arg a "$ADMIN_ADDRESS" '[$a]')" ]] \
    || die "admin check failed: expected [$ADMIN_ADDRESS], got $admins — do not use this contract"
}
verify_template_uri() { # id expected_uri
  local got
  got="$(invoke get_template --template_id "$1" | jq -r .metadata_uri)"
  [[ "$got" == "$2" ]] || die "template $1 is registered with $got, but $MANIFEST has $2"
}
manifest_entries() { [[ -z "$MANIFEST" ]] || jq -r '.[] | [.template_id, .metadata_uri] | @tsv' "$MANIFEST"; }
# Templates already on-chain (registered by an earlier run or by hand) must
# carry the manifest URI; they are marked done instead of registered again.
sync_templates() {
  [[ -n "$MANIFEST" ]] || return 0
  local on_chain id uri
  on_chain="$(invoke list_templates)"
  while IFS=$'\t' read -r id uri; do
    if jq -e --argjson id "$id" 'any(.[]; . == $id)' <<<"$on_chain" >/dev/null; then
      verify_template_uri "$id" "$uri"
      step_done "template-$id" || record_step "template-$id" already-registered
    fi
  done < <(manifest_entries)
}
verify_templates() {
  [[ -n "$MANIFEST" ]] || return 0
  local on_chain
  on_chain="$(invoke list_templates)"
  [[ "$(jq -c 'sort' <<<"$on_chain")" == "$(jq -c '[.[].template_id] | sort' "$MANIFEST")" ]] \
    || die "list_templates returned $on_chain, expected ids from $MANIFEST"
}
next_step() {
  local s id uri
  for s in upload_wasm deploy initialize; do
    step_done "$s" || { echo "$s"; return 0; }
  done
  while IFS=$'\t' read -r id uri; do
    step_done "template-$id" || { echo "template-$id"; return 0; }
  done < <(manifest_entries)
}
template_uri() { jq -r --argjson id "$1" '.[] | select(.template_id == $id) | .metadata_uri' "$MANIFEST"; }

# --- --build-only: unsigned XDR of the next step ---------------------------
if [[ "$MODE" == "build" ]]; then
  if step_done initialize; then sync_templates; fi
  STEP="$(next_step)"
  [[ -n "$STEP" ]] || die "every step is done; nothing to build"
  case "$STEP" in
    upload_wasm)
      OP="upload wasm"
      ARGS="wasm $WASM (sha256 $WASM_HASH)"
      XDR="$(run_stellar contract upload --wasm "$WASM" --source "$SOURCE" "${NET_ARGS[@]}" --build-only)"
      ;;
    deploy)
      SALT="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
      CONTRACT_ID="$(run_stellar contract id wasm --salt "$SALT" --source-account "$SOURCE" "${NET_ARGS[@]}")"
      [[ "$CONTRACT_ID" =~ ^C[A-Z2-7]{55}$ ]] || die "unexpected contract id: $CONTRACT_ID"
      state_set '.contract_id = $c | .salt = $s' --arg c "$CONTRACT_ID" --arg s "$SALT"
      OP="deploy contract $CONTRACT_ID"
      ARGS="wasm hash $WASM_HASH, salt $SALT"
      XDR="$(run_stellar contract deploy --wasm-hash "$WASM_HASH" --salt "$SALT" --source "$SOURCE" "${NET_ARGS[@]}" --build-only)"
      ;;
    initialize)
      OP="invoke $CONTRACT_ID initialize"
      ARGS="--admin $ADMIN_ADDRESS"
      XDR="$(invoke initialize --admin "$ADMIN_ADDRESS" --build-only)"
      ;;
    template-*)
      ID="${STEP#template-}"
      OP="invoke $CONTRACT_ID register_template"
      ARGS="--admin $ADMIN_ADDRESS --template_id $ID --metadata_uri $(template_uri "$ID")"
      XDR="$(invoke register_template --admin "$ADMIN_ADDRESS" --template_id "$ID" --metadata_uri "$(template_uri "$ID")" --build-only)"
      ;;
  esac
  # Simulation adds the Soroban footprint and resource fee so signers sign the final tx.
  XDR="$(printf '%s' "$XDR" | run_stellar tx simulate --source "$SOURCE" "${NET_ARGS[@]}")"
  FEE="$(printf '%s' "$XDR" | stellar tx decode --output json 2>/dev/null | jq -r '[.. | objects | .fee? // empty] | first // "unknown"')" || FEE="unknown"
  printf '%s\n' "$XDR" >"$STATE_DIR/$STEP.xdr"
  cat >"$STATE_DIR/$STEP.txt" <<SUMMARY
step:      $STEP
network:   $NETWORK ($PASSPHRASE)
source:    $SOURCE
operation: $OP
args:      $ARGS
fee:       $FEE stroops
SUMMARY
  state_set '.pending = $s' --arg s "$STEP"
  cat "$STATE_DIR/$STEP.txt"
  echo "==> Wrote $STATE_DIR/$STEP.xdr. Sign it (e.g. stellar tx sign), then run:"
  echo "    ADMIN_ADDRESS=$ADMIN_ADDRESS $0 $NETWORK --submit <signed.xdr>"
  exit 0
fi

# --- --submit: send the signed pending step and verify it --------------------
if [[ "$MODE" == "submit" ]]; then
  STEP="$(state .pending)"
  [[ -n "$STEP" ]] || die "no pending step in $STATE; run --build-only first"
  TX_HASH="$(run_stellar tx hash "${NET_ARGS[@]}" <"$SIGNED_XDR")"
  echo "==> Submitting $STEP (tx $TX_HASH)"
  run_stellar tx send "${NET_ARGS[@]}" <"$SIGNED_XDR" >/dev/null
  case "$STEP" in
    upload_wasm) verify_fetched_wasm --wasm-hash "$WASM_HASH" ;;
    deploy) verify_fetched_wasm --id "$CONTRACT_ID" ;;
    initialize) verify_admins ;;
    template-*) verify_template_uri "${STEP#template-}" "$(template_uri "${STEP#template-}")" ;;
  esac
  record_step "$STEP" "$TX_HASH"
  if [[ -n "$(next_step)" ]]; then
    echo "==> $STEP done. Next: $(next_step) (run --build-only)"
    exit 0
  fi
  verify_admins
  verify_templates
fi

# --- local signing: run every pending step ----------------------------------
if [[ "$MODE" == "local" ]]; then
  if ! step_done upload_wasm; then
    echo "==> Uploading WASM to $NETWORK"
    UPLOADED="$(run_stellar contract upload --wasm "$WASM" --source "$SOURCE" "${NET_ARGS[@]}")"
    [[ "$UPLOADED" == "$LOCAL_HASH" ]] || die "uploaded wasm hash $UPLOADED != local $LOCAL_HASH"
    UPLOAD_TX="$(last_tx)"
    record_step upload_wasm "${UPLOAD_TX:-already-uploaded}"
  fi

  if ! step_done deploy; then
    echo "==> Deploying contract"
    CONTRACT_ID="$(run_stellar contract deploy --wasm-hash "$WASM_HASH" --source "$SOURCE" "${NET_ARGS[@]}")"
    [[ "$CONTRACT_ID" =~ ^C[A-Z2-7]{55}$ ]] || die "unexpected contract id: $CONTRACT_ID"
    state_set '.contract_id = $c' --arg c "$CONTRACT_ID"
    record_step deploy "$(last_tx)"
  fi
  echo "    contract id: $CONTRACT_ID"

  if ! step_done initialize; then
    echo "==> Initializing with admin $ADMIN_ADDRESS"
    # initialize is a separate transaction from deploy. If anyone front-runs it,
    # this call fails with AlreadyInitialized (#1) and the admin check below fails.
    invoke initialize --admin "$ADMIN_ADDRESS" >/dev/null
    record_step initialize "$(last_tx)"
  fi
  verify_admins

  if [[ -n "$MANIFEST" ]]; then
    echo "==> Registering templates from $MANIFEST"
    sync_templates
    while IFS=$'\t' read -r id uri; do
      if step_done "template-$id"; then
        echo "    template $id already registered"
        continue
      fi
      invoke register_template --admin "$ADMIN_ADDRESS" --template_id "$id" --metadata_uri "$uri" >/dev/null
      echo "    template $id -> $uri (tx $(last_tx))"
      record_step "template-$id" "$(last_tx)"
    done < <(manifest_entries)
    verify_templates
  fi
fi

# --- deployment record ----------------------------------------------------
TEMPLATES_JSON="$(manifest_entries | jq -R -s -c --slurpfile st "$STATE" '
  [split("\n")[] | select(length > 0) | split("\t") | (.[0] | tonumber) as $id
   | $st[0].steps["template-" + .[0]] as $s
   | {template_id: $id, metadata_uri: .[1], tx_hash: $s.tx, ledger: $s.ledger}]')"

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
  --slurpfile state "$STATE" \
  --argjson templates "$TEMPLATES_JSON" \
  '$state[0].steps as $s | {
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
    deploy_ledger: $s.deploy.ledger,
    transactions: {upload_wasm: $s.upload_wasm.tx, deploy: $s.deploy.tx, initialize: $s.initialize.tx},
    ledgers: {upload_wasm: $s.upload_wasm.ledger, deploy: $s.deploy.ledger, initialize: $s.initialize.ledger},
    templates: $templates
  }' >"$OUT.tmp"
python3 -I scripts/validate_deployments.py "$OUT.tmp" >/dev/null || die "generated record fails deployments/schema.json; see $OUT.tmp"
mv "$OUT.tmp" "$OUT"

echo "==> Done. Wrote $OUT"
echo "CONTRACT_ID=$CONTRACT_ID"
echo "WASM_HASH=$WASM_HASH"
echo "DEPLOY_LEDGER=$(jq -r .deploy_ledger "$OUT")"
