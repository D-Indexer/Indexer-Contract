#!/usr/bin/env python3
"""Validation helpers for scripts/verify-deployment.sh."""
import json
import re
import sys
from pathlib import Path
from typing import Any


HASH_RE = re.compile(r"^[0-9a-fA-F]{64}$")


def load_deployment(path: Path) -> dict[str, Any]:
    try:
        record = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read deployment record {path}: {error}") from error
    if not isinstance(record, dict):
        raise ValueError("deployment record must be a JSON object")
    for field in ("contract_id", "source_commit", "wasm_hash"):
        if not isinstance(record.get(field), str) or not record[field]:
            raise ValueError(f"deployment record is missing {field}")
    toolchain = record.get("toolchain")
    if not isinstance(toolchain, dict):
        raise ValueError("deployment record is missing toolchain")
    for field in ("rustc", "stellar_cli"):
        if not isinstance(toolchain.get(field), str) or not toolchain[field]:
            raise ValueError(f"deployment record is missing toolchain.{field}")
    return record


def check_toolchain(
    record: dict[str, Any], rustc_version: str, stellar_version: str
) -> bool:
    expected = record["toolchain"]
    matched = True
    for label, recorded, actual in (
        ("rustc", expected["rustc"], rustc_version),
        ("stellar", expected["stellar_cli"], stellar_version),
    ):
        if recorded != actual:
            print(f"FAIL {label} version mismatch: expected '{recorded}', got '{actual}'", file=sys.stderr)
            matched = False
        else:
            print(f"OK {label}: {actual}")
    return matched


def hashes_match(expected: str, actual: str) -> bool:
    if not HASH_RE.fullmatch(expected) or not HASH_RE.fullmatch(actual):
        raise ValueError("WASM hashes must be 64 hexadecimal characters")
    return expected.lower() == actual.lower()


def main(argv: list[str]) -> int:
    if len(argv) == 5 and argv[1] == "check-toolchain":
        try:
            record = load_deployment(Path(argv[2]))
        except ValueError as error:
            print(f"verify-deployment: {error}", file=sys.stderr)
            return 2
        return 0 if check_toolchain(record, argv[3], argv[4]) else 1
    if len(argv) == 4 and argv[1] == "compare-hash":
        try:
            matched = hashes_match(argv[2], argv[3])
        except ValueError as error:
            print(f"verify-deployment: {error}", file=sys.stderr)
            return 2
        if matched:
            print(f"PASS wasm sha256 {argv[3]}")
            return 0
        print(
            f"FAIL wasm sha256 mismatch: expected {argv[2]}, got {argv[3]}",
            file=sys.stderr,
        )
        return 1
    print(
        "Usage: verify_deployment.py check-toolchain RECORD RUSTC_VERSION STELLAR_VERSION"
        "\n       verify_deployment.py compare-hash EXPECTED ACTUAL",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
