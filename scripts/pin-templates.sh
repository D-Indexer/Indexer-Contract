#!/usr/bin/env bash
# pin-templates.sh — pin every template in templates/manifest.json and verify
# that each CID is retrievable, byte-for-byte, from at least two gateways.
#
# Usage: ./scripts/pin-templates.sh [--verify-only]
# Env:   PIN_PROVIDER   pinning service: pinata (default)
#        PIN_API_URL    override the provider endpoint
#        PIN_TOKEN      provider JWT/API token (required unless --verify-only;
#                       read from the environment / CI secret, never committed)
#        GATEWAYS       space-separated gateway hosts or base URLs
#                       (default: "ipfs.io dweb.link")
#        MIN_GATEWAYS   gateways that must serve each CID (default: 2)
#        RETRIES        attempts per gateway (default: 4)
#        FETCH_TIMEOUT  seconds per request (default: 30)
#        MANIFEST       manifest path (default: templates/manifest.json)
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERIFY_ONLY=0
[[ "${1:-}" == "--verify-only" ]] && VERIFY_ONLY=1

MANIFEST="${MANIFEST:-templates/manifest.json}"
PIN_PROVIDER="${PIN_PROVIDER:-pinata}"
read -r -a GATEWAY_LIST <<<"${GATEWAYS:-ipfs.io dweb.link}"
MIN_GATEWAYS="${MIN_GATEWAYS:-2}"
RETRIES="${RETRIES:-4}"
FETCH_TIMEOUT="${FETCH_TIMEOUT:-30}"

for cmd in curl jq sha256sum; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "missing required command: $cmd" >&2; exit 1; }
done

pin() {
  local file="$1" cid="$2" got
  case "$PIN_PROVIDER" in
    pinata)
      got="$(curl -sSf --max-time "$FETCH_TIMEOUT" \
        -H "Authorization: Bearer $PIN_TOKEN" \
        -F "file=@$file" -F 'pinataOptions={"cidVersion":1}' \
        "${PIN_API_URL:-https://api.pinata.cloud/pinning/pinFileToIPFS}" | jq -r .IpfsHash)"
      ;;
    *) echo "unsupported PIN_PROVIDER: $PIN_PROVIDER" >&2; return 1 ;;
  esac
  [[ "$got" == "$cid" ]] || { echo "pinned $file as $got, manifest says $cid" >&2; return 1; }
  echo "pinned $file -> $cid"
}

# fetch_ok <gateway> <cid> <expected sha256>
fetch_ok() {
  local gw="$1" cid="$2" want="$3" base url got delay=1 attempt tmp
  [[ "$gw" == http://* || "$gw" == https://* ]] && base="$gw" || base="https://$gw"
  url="$base/ipfs/$cid"
  tmp="$(mktemp)"
  for ((attempt = 1; attempt <= RETRIES; attempt++)); do
    if curl -sSfL --max-time "$FETCH_TIMEOUT" -o "$tmp" "$url" 2>/dev/null; then
      got="$(sha256sum "$tmp" | cut -d' ' -f1)"
      if [[ "$got" == "$want" ]]; then rm -f "$tmp"; return 0; fi
      echo "  $url: sha256 mismatch ($got)" >&2
    else
      echo "  $url: attempt $attempt failed" >&2
    fi
    ((attempt < RETRIES)) && sleep "$delay" && delay=$((delay * 2))
  done
  rm -f "$tmp"
  return 1
}

if ((VERIFY_ONLY == 0)) && [[ -z "${PIN_TOKEN:-}" ]]; then
  echo "PIN_TOKEN is required (or pass --verify-only)" >&2
  exit 1
fi

FAILED=0
while IFS=$'\t' read -r file cid; do
  if ((VERIFY_ONLY == 0)); then pin "$file" "$cid" || { FAILED=1; continue; }; fi
  want="$(sha256sum "$file" | cut -d' ' -f1)"
  ok=0
  for gw in "${GATEWAY_LIST[@]}"; do
    if fetch_ok "$gw" "$cid" "$want"; then ok=$((ok + 1)); fi
  done
  if ((ok >= MIN_GATEWAYS)); then
    echo "OK   $file $cid ($ok/${#GATEWAY_LIST[@]} gateways)"
  else
    echo "FAIL $file $cid ($ok/${#GATEWAY_LIST[@]} gateways, need $MIN_GATEWAYS)" >&2
    FAILED=1
  fi
done < <(jq -r '.[] | [.file, .cid] | @tsv' "$MANIFEST")

exit "$FAILED"
