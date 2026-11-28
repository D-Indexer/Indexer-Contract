#!/usr/bin/env bash
# Tests testnet-reset-check.sh with a mocked stellar CLI.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MOCK="$(mktemp -d)"
trap 'rm -rf "$MOCK"' EXIT

cat >"$MOCK/stellar" <<'SH'
#!/usr/bin/env bash
case "$MOCK_MODE" in
  exists)  echo "interface ..."; exit 0 ;;
  missing) echo "error: Contract not found: $4" >&2; exit 1 ;;
  rpc)     echo "error: connection refused" >&2; exit 1 ;;
esac
SH
chmod +x "$MOCK/stellar"

fail=0
expect() { # expect <mode> <exit code> <exists value or ->
  local out code=0
  out="$(MOCK_MODE="$1" STELLAR="$MOCK/stellar" GITHUB_OUTPUT="$MOCK/out" \
    "$ROOT/scripts/testnet-reset-check.sh" 2>/dev/null)" || code=$?
  if [[ "$code" == "$2" ]] && { [[ "$3" == "-" ]] || grep -qx "exists=$3" <<<"$out"; }; then
    echo "ok   $1"
  else
    echo "FAIL $1 (exit $code, output: $out)"; fail=1
  fi
}
expect exists  0 true
expect missing 0 false
expect rpc     2 -
exit "$fail"
