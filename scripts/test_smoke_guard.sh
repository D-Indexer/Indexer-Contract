#!/usr/bin/env bash
# Network-guard tests for smoke-testnet.sh. A stub `stellar` on PATH records
# any invocation, so a refused run is proven to make no network calls.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir "$TMP/bin"
printf '#!/bin/sh\necho "$@" >>"%s/calls"\nexit 1\n' "$TMP" >"$TMP/bin/stellar"
chmod +x "$TMP/bin/stellar"

deployment() { jq -n --arg n "$1" '{network: $n, rpc_url: "http://127.0.0.1:1", network_passphrase: "x", contract_id: "C", templates: [{template_id: 1}]}' >"$TMP/$1.json"; }
deployment mainnet
deployment futurenet

fails=0
expect() { # expect <name> <exit code> <stderr pattern> -- args...
  local name="$1" code="$2" pattern="$3"; shift 4
  rm -f "$TMP/calls"
  set +e
  err="$(PATH="$TMP/bin:$PATH" "$@" 2>&1 >/dev/null)"; rc=$?
  set -e
  if [[ "$rc" == "$code" && "$err" == *"$pattern"* ]]; then
    echo "ok   $name"
  else
    echo "FAIL $name (rc=$rc, stderr: $err)"; fails=$((fails + 1))
  fi
}

S="$ROOT/scripts/smoke-testnet.sh"
expect "mainnet refused without --simulate" 1 "refusing to run against mainnet without --simulate" -- env DEPLOYMENT="$TMP/mainnet.json" "$S"
[[ ! -f "$TMP/calls" ]] || { echo "FAIL stellar was invoked before the guard"; fails=$((fails + 1)); }
expect "unknown network refused" 1 "refusing to smoke-test network 'futurenet'" -- env DEPLOYMENT="$TMP/futurenet.json" "$S" --simulate
expect "mainnet simulate needs SIM_SOURCE" 1 "SIM_SOURCE (a funded account) is required on mainnet" -- env -u SIM_SOURCE DEPLOYMENT="$TMP/mainnet.json" "$S" --simulate
expect "unknown argument rejected" 2 "unknown argument: --send" -- "$S" --send

[[ "$fails" == 0 ]] || exit 1
echo "all smoke guard tests passed"
