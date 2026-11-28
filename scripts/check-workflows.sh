#!/usr/bin/env bash
# Fails if any workflow uses an action not pinned to a full commit SHA,
# or lacks a top-level `permissions:` block.
set -euo pipefail

status=0
for f in .github/workflows/*.yml .github/workflows/*.yaml; do
  [[ -e "$f" ]] || continue
  if ! grep -qE '^permissions:' "$f"; then
    echo "::error file=$f::missing top-level permissions"
    status=1
  fi
  while IFS= read -r line; do
    echo "::error file=$f::action not pinned to a commit SHA: $line"
    status=1
  done < <(grep -nE '^\s*(-\s+)?uses:\s*' "$f" | grep -vE 'uses:\s*\./' | grep -vE '@[0-9a-f]{40}(\s|$)' || true)
done
exit "$status"
