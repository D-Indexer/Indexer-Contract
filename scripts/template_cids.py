#!/usr/bin/env python3
"""Compute or verify the IPFS CIDs and SHA-256 digests of template metadata.

The CID is a CIDv1 (raw codec, sha2-256, base32) of the file bytes, which is
what `ipfs add --cid-version 1 <file>` produces for files under the 256 KiB
chunk size. Standard library only.

Usage:
  scripts/template_cids.py <file>...        print "<cid> <sha256>  <file>" per file
  scripts/template_cids.py --check          verify CIDs and hashes in manifest.json
"""
import base64
import hashlib
import json
import pathlib
import sys

RAW_CODEC = 0x55
SHA2_256 = 0x12
CHUNK_LIMIT = 256 * 1024


def cid_v1_raw(data: bytes) -> str:
    if len(data) > CHUNK_LIMIT:
        raise ValueError("file exceeds a single IPFS chunk; CID would be a dag-pb root")
    multihash = bytes([SHA2_256, 32]) + hashlib.sha256(data).digest()
    body = base64.b32encode(bytes([1, RAW_CODEC]) + multihash).decode().lower().rstrip("=")
    return "b" + body


def check(manifest_path: pathlib.Path) -> int:
    root = manifest_path.parent.parent
    entries = json.loads(manifest_path.read_text())
    failed = False
    for entry in entries:
        data = (root / entry["file"]).read_bytes()
        actual = cid_v1_raw(data)
        actual_hash = hashlib.sha256(data).hexdigest()
        ok = (
            actual == entry["cid"]
            and entry["metadata_uri"] == f"ipfs://{actual}"
            and actual_hash == entry["content_sha256"]
        )
        print(
            f"{'ok ' if ok else 'BAD'} template {entry['template_id']}: "
            f"{actual} sha256={actual_hash}"
        )
        failed |= not ok
    return 1 if failed else 0


def main(argv: list[str]) -> int:
    if argv[1:] == ["--check"]:
        return check(pathlib.Path(__file__).resolve().parent.parent / "templates" / "manifest.json")
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    for name in argv[1:]:
        data = pathlib.Path(name).read_bytes()
        print(f"{cid_v1_raw(data)} {hashlib.sha256(data).hexdigest()}  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
