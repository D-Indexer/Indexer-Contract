#!/usr/bin/env bash
# sdk-compat.sh — compatibility report for a soroban-sdk (or toolchain) bump.
#
# Compares a base git ref (before the bump) with the working tree (after it):
#   - interface: `stellar contract info interface` of the release wasm
#     (skipped with a note if the stellar CLI is not installed);
#   - event XDR fixtures and auth trees recorded in test_snapshots/;
#   - wasm size (`stellar contract build`);
#   - per-entry-point CPU/memory budgets (`budget_report` test).
#
# Usage: ./scripts/sdk-compat.sh [BASE_REF]   (default: origin/main)
# Prints a Markdown report to stdout; paste it into the upgrade PR.
set -Eeuo pipefail
trap 'echo "sdk-compat.sh: FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_REF="${1:-origin/main}"
WORK="$(mktemp -d)"
cleanup() {
  git -C "$ROOT" worktree remove --force "$WORK/base-src" >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

# collect <source dir> <output dir>
collect() {
  local src="$1" out="$2"
  mkdir -p "$out"
  (
    cd "$src"
    grep -m1 '^soroban-sdk' Cargo.toml >"$out/sdk.txt"
    # soroban-sdk >= 26 refuses plain `cargo build` for wasm; use the CLI.
    if command -v stellar >/dev/null; then
      local wasm=target/compat/indexer_contract.wasm
      stellar contract build --locked --out-dir target/compat >/dev/null
      stat -c %s "$wasm" >"$out/wasm_size.txt"
      stellar contract info interface --wasm "$wasm" >"$out/interface.txt"
    else
      echo "n/a" >"$out/wasm_size.txt"
      echo "stellar CLI not installed; interface not compared" >"$out/interface.txt"
    fi
    cargo test --quiet --locked >/dev/null 2>&1 || echo "tests FAILED" >"$out/tests_failed.txt"
    cargo test --quiet --locked budget_report -- --ignored --nocapture 2>/dev/null \
      | grep '^BUDGET' >"$out/budgets.txt" || echo "budget_report test not present" >"$out/budgets.txt"
    rm -f test_snapshots/test/budget_report.1.json
    # Keep only the parts the backend depends on: events and auth trees.
    find test_snapshots -name '*.json' | sort | while read -r f; do
      mkdir -p "$out/$(dirname "$f")"
      jq -S '{auth, events}' "$f" >"$out/$f"
    done
  )
}

git -C "$ROOT" worktree add --quiet --detach "$WORK/base-src" "$BASE_REF"
collect "$WORK/base-src" "$WORK/base" >&2
collect "$ROOT" "$WORK/head" >&2

section() { echo; echo "### $1"; echo; }
code_diff() {
  if diff -u "$1" "$2" >"$WORK/d"; then echo "No changes."; else
    echo '```diff'; sed "s#$WORK/##g" "$WORK/d"; echo '```'; fi
}

echo "## SDK compatibility report"
echo
echo "- base (\`$BASE_REF\`): \`$(cat "$WORK/base/sdk.txt")\`"
echo "- head (working tree): \`$(cat "$WORK/head/sdk.txt")\`"
[[ -f "$WORK/head/tests_failed.txt" ]] && echo "- ❌ tests fail on head" || echo "- ✅ tests pass on head"

section "Wasm size (stellar contract build, unoptimized)"
b=$(cat "$WORK/base/wasm_size.txt"); h=$(cat "$WORK/head/wasm_size.txt")
d="n/a"; [[ "$b" =~ ^[0-9]+$ && "$h" =~ ^[0-9]+$ ]] && d=$((h - b))
echo "| base | head | delta |"; echo "|---|---|---|"; echo "| $b | $h | $d |"

section "Interface"
code_diff "$WORK/base/interface.txt" "$WORK/head/interface.txt"

section "Event XDR fixtures and auth snapshots"
if diff -rq "$WORK/base/test_snapshots" "$WORK/head/test_snapshots" >"$WORK/snap"; then
  echo "No changes."
else
  echo '```'; sed "s#$WORK/##g" "$WORK/snap"; echo '```'
  diff -ru "$WORK/base/test_snapshots" "$WORK/head/test_snapshots" >"$WORK/snapdiff" || true
  echo; echo "<details><summary>Full diff</summary>"; echo
  echo '```diff'; sed "s#$WORK/##g" "$WORK/snapdiff"; echo '```'; echo "</details>"
fi

section "Budgets (native test env, per entry point)"
code_diff "$WORK/base/budgets.txt" "$WORK/head/budgets.txt"
