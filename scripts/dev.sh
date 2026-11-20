#!/usr/bin/env bash
# dev.sh — one-command local environment with seeded data.
#
# Usage: ./scripts/dev.sh up|down|verify
#
#   up      Start a local Stellar network container, build and deploy the
#           contract, register the templates and seed DEV_USERS users.
#           Idempotent: re-running reuses the deployment in
#           deployments/local.json and skips anything already on-chain.
#   down    Stop the local network container and remove deployments/local.json.
#   verify  Check the seeded state in deployments/local.json through reads.
#
# Environment (all optional):
#   DEV_USERS          Number of users to seed (default: 25).
#   STELLAR_RPC_URL    Local RPC endpoint (default: http://localhost:8000/rpc).
set -Eeuo pipefail
trap 'echo "dev.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

CMD="${1:-up}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

N="${DEV_USERS:-25}"
RPC_URL="${STELLAR_RPC_URL:-http://localhost:8000/rpc}"
PASSPHRASE="Standalone Network ; February 2017"
NET_ARGS=(--network local --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
ADMIN="folder-dev-admin"
WASM="target/stellar/indexer_contract.wasm"
MANIFEST="templates/manifest.json"
OUT="deployments/local.json"

die() { echo "dev.sh: $*" >&2; exit 1; }
for cmd in stellar jq python3 curl; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

# Deterministic per-name 32-char (256-bit) seed, so identities (and addresses) are stable.
seed_for() { python3 -Ic 'import hashlib,sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest()[:32])' "$1"; }
# Deterministic, validly shaped CIDv1 (`bafkrei` + base32 of a sha256).
cid_for() {
  python3 -Ic 'import base64,hashlib,sys; print("bafkrei" + base64.b32encode(hashlib.sha256(sys.argv[1].encode()).digest()).decode().lower().rstrip("="))' "$1"
}
user_name() { printf 'folder-dev-user-%03d' "$1"; }
folder_for() { printf 'user%03d.folder' "$1"; }
template_for() { echo $(( ($1 - 1) % $(jq length "$MANIFEST") + 1 )); }

ensure_identity() {
  local name="$1"
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    stellar keys generate "$name" --seed "$(seed_for "$name")" "${NET_ARGS[@]}" >/dev/null 2>&1
  fi
  stellar keys fund "$name" "${NET_ARGS[@]}" >/dev/null 2>&1 || true
}

invoke() {
  local source="$1"; shift
  stellar contract invoke --id "$CONTRACT_ID" --source "$source" "${NET_ARGS[@]}" -- "$@" 2>/dev/null
}

wait_for_rpc() {
  echo "==> Waiting for local RPC at $RPC_URL"
  for _ in $(seq 1 120); do
    if curl -s -X POST -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$RPC_URL" | grep -q '"healthy"'; then
      return 0
    fi
    sleep 2
  done
  die "local RPC did not become healthy"
}

up() {
  echo "==> Starting local network container"
  stellar container start local >/dev/null 2>&1 || echo "    (container already running)"
  wait_for_rpc

  ensure_identity "$ADMIN"
  ADMIN_ADDRESS="$(stellar keys address "$ADMIN")"

  CONTRACT_ID=""
  if [[ -f "$OUT" ]]; then
    CONTRACT_ID="$(jq -r .contract_id "$OUT")"
    if invoke "$ADMIN" get_admins >/dev/null; then
      echo "==> Reusing contract $CONTRACT_ID"
    else
      CONTRACT_ID=""
    fi
  fi

  if [[ -z "$CONTRACT_ID" ]]; then
    echo "==> Building optimized WASM"
    make -s build-optimized >/dev/null
    echo "==> Deploying"
    CONTRACT_ID="$(stellar contract deploy --wasm "$WASM" --source "$ADMIN" "${NET_ARGS[@]}" 2>/dev/null)"
    invoke "$ADMIN" initialize --admin "$ADMIN_ADDRESS" >/dev/null
  fi

  echo "==> Registering templates"
  while IFS=$'\t' read -r id uri; do
    invoke "$ADMIN" get_template --template_id "$id" >/dev/null \
      || invoke "$ADMIN" register_template --admin "$ADMIN_ADDRESS" --template_id "$id" --metadata_uri "$uri" >/dev/null
  done < <(jq -r '.[] | [.template_id, .metadata_uri] | @tsv' "$MANIFEST")

  echo "==> Seeding $N users"
  local users="[]"
  for i in $(seq 1 "$N"); do
    local name folder tmpl cid addr
    name="$(user_name "$i")"; folder="$(folder_for "$i")"
    tmpl="$(template_for "$i")"; cid="$(cid_for "$name")"
    ensure_identity "$name"
    addr="$(stellar keys address "$name")"
    invoke "$name" get_identity --owner "$addr" >/dev/null \
      || invoke "$name" claim_folder --owner "$addr" --folder_name "$folder" >/dev/null
    invoke "$name" get_portfolio --owner "$addr" >/dev/null \
      || invoke "$name" mint_portfolio --owner "$addr" --template_id "$tmpl" --ipfs_cid "$cid" >/dev/null
    [[ "$(invoke "$name" get_user_template --owner "$addr")" == "$tmpl" ]] \
      || invoke "$name" set_user_template --owner "$addr" --template_id "$tmpl" >/dev/null
    users="$(jq -c --arg a "$addr" --arg f "$folder" --argjson t "$tmpl" --arg c "$cid" \
      '. + [{address: $a, folder: $f, template_id: $t, ipfs_cid: $c}]' <<<"$users")"
  done

  mkdir -p deployments
  jq -n --arg rpc "$RPC_URL" --arg pp "$PASSPHRASE" --arg id "$CONTRACT_ID" \
    --arg admin "$ADMIN_ADDRESS" --argjson users "$users" \
    '{network: "local", rpc_url: $rpc, network_passphrase: $pp, contract_id: $id,
      admin: $admin, users: $users}' >"$OUT"

  echo "==> Done. Wrote $OUT. Env vars for the other repos:"
  echo "CONTRACT_ID=$CONTRACT_ID"
  echo "STELLAR_NETWORK=local"
  echo "STELLAR_RPC_URL=$RPC_URL"
  echo "STELLAR_NETWORK_PASSPHRASE=\"$PASSPHRASE\""
}

verify() {
  [[ -f "$OUT" ]] || die "$OUT not found; run 'make dev' first"
  CONTRACT_ID="$(jq -r .contract_id "$OUT")"
  local count=0
  while IFS=$'\t' read -r addr folder tmpl cid; do
    [[ "$(invoke "$ADMIN" get_identity --owner "$addr" | jq -r .)" == "$folder" ]] || die "identity mismatch for $addr"
    [[ "$(invoke "$ADMIN" get_folder --folder_name "$folder" | jq -r .)" == "$addr" ]] || die "folder mismatch for $folder"
    local p
    p="$(invoke "$ADMIN" get_portfolio --owner "$addr")"
    [[ "$(jq -r .ipfs_cid <<<"$p")" == "$cid" && "$(jq -r .template_id <<<"$p")" == "$tmpl" ]] \
      || die "portfolio mismatch for $addr: $p"
    count=$((count + 1))
  done < <(jq -r '.users[] | [.address, .folder, .template_id, .ipfs_cid] | @tsv' "$OUT")
  [[ "$count" -eq "$N" ]] || die "expected $N seeded users, found $count"
  [[ "$(invoke "$ADMIN" list_templates | jq -c sort)" == "$(jq -c '[.[].template_id] | sort' "$MANIFEST")" ]] \
    || die "template list mismatch"
  echo "==> Verified $count seeded users"
}

down() {
  echo "==> Stopping local network container"
  stellar container stop local >/dev/null 2>&1 || true
  rm -f "$OUT"
}

case "$CMD" in
  up) up ;;
  down) down ;;
  verify) verify ;;
  *) die "unknown command: $CMD (expected up|down|verify)" ;;
esac
