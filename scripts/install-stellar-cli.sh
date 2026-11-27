#!/usr/bin/env bash
# Install the Stellar CLI after verifying the tarball against a sha256 pinned
# in this repository. The pin is never read from the release itself: a swapped
# tarball and checksums file must still fail. Update version, sha256 and size
# together, as one named change.
set -euo pipefail

: "${STELLAR_CLI_VERSION:?STELLAR_CLI_VERSION is required}"
: "${STELLAR_CLI_SHA256:?STELLAR_CLI_SHA256 is required}"
DEST="${1:-${RUNNER_TEMP:-/usr/local/bin}}"

TARBALL="stellar-cli-${STELLAR_CLI_VERSION}-x86_64-unknown-linux-gnu.tar.gz"
URL="https://github.com/stellar/stellar-cli/releases/download/v${STELLAR_CLI_VERSION}/${TARBALL}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -sSfL -o "$TMP/$TARBALL" "$URL"

if ! echo "${STELLAR_CLI_SHA256}  $TMP/$TARBALL" | sha256sum --check --status; then
  echo "::error::Stellar CLI ${STELLAR_CLI_VERSION} checksum mismatch" >&2
  echo "  expected: ${STELLAR_CLI_SHA256}" >&2
  echo "  actual:   $(sha256sum "$TMP/$TARBALL" | cut -d' ' -f1)" >&2
  echo "Refusing to extract an unverified tarball." >&2
  exit 1
fi

tar xzf "$TMP/$TARBALL" -C "$DEST" stellar
"$DEST/stellar" --version
