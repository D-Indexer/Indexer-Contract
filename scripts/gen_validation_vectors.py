#!/usr/bin/env python3
"""Deterministically generate fixtures/validation-vectors.json.

Mirrors the rules in docs/VALIDATION.md and src/validation.rs. Run from the
repo root: `python3 scripts/gen_validation_vectors.py`. Output is stable: no
randomness, no timestamps, sorted by (kind, input).

Each vector: {"kind": "folder"|"cid"|"uri"|"template_id", "input": str,
"expected": "ok" | "<ContractErrorName>"}.
"""

import json
import os

FOLDER_SUFFIX = ".folder"
MIN_FOLDER_LABEL_LEN = 3
MAX_FOLDER_NAME_LEN = 64
CIDV0_LEN = 46
MIN_IPFS_CID_LEN = 46
MAX_IPFS_CID_LEN = 128
METADATA_URI_SCHEME = "ipfs://"
MAX_METADATA_URI_LEN = 256

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE32_LOWER = "abcdefghijklmnopqrstuvwxyz234567"
PATH_BYTES = set(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._~/-"
)

# Well-formed bases: CIDv1 (base32, 59 chars) and CIDv0 (46 chars).
CIDV1 = "bafkreibm6jg3ux5qumhcn2b3flc3tyu6dmlb4xa7u5bf44yegnrjhc4yeq"
CIDV0 = "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"


def is_label_byte(ch):
    return ("a" <= ch <= "z") or ("0" <= ch <= "9") or ch == "-"


def is_valid_cid(s):
    if s.startswith("Qm"):
        return len(s) == CIDV0_LEN and all(c in BASE58 for c in s)
    if s.startswith("ba"):
        return MIN_IPFS_CID_LEN <= len(s) <= MAX_IPFS_CID_LEN and all(
            c in BASE32_LOWER for c in s
        )
    return False


def check_folder(s):
    if len(s) > MAX_FOLDER_NAME_LEN:
        return "InvalidFolderName"
    if len(s) < MIN_FOLDER_LABEL_LEN + len(FOLDER_SUFFIX):
        return "InvalidFolderName"
    if not s.endswith(FOLDER_SUFFIX):
        return "InvalidFolderName"
    label = s[: len(s) - len(FOLDER_SUFFIX)]
    if not all(is_label_byte(c) for c in label):
        return "InvalidFolderName"
    if label.startswith("-") or label.endswith("-"):
        return "InvalidFolderName"
    return "ok"


def check_cid(s):
    return "ok" if is_valid_cid(s) else "InvalidCID"


def check_uri(s):
    if len(s) > MAX_METADATA_URI_LEN or not s.startswith(METADATA_URI_SCHEME):
        return "InvalidMetadataURI"
    rest = s[len(METADATA_URI_SCHEME):]
    idx = rest.find("/")
    if idx == -1:
        cid, path = rest, ""
    else:
        cid, path = rest[:idx], rest[idx:]
    if is_valid_cid(cid) and all(c in PATH_BYTES for c in path):
        return "ok"
    return "InvalidMetadataURI"


def check_template_id(s):
    try:
        value = int(s)
    except ValueError:
        return "InvalidTemplateID"
    if value < 0 or value > 0xFFFFFFFF:
        return "InvalidTemplateID"
    return "ok" if value != 0 else "InvalidTemplateID"


CHECKS = {
    "folder": check_folder,
    "cid": check_cid,
    "uri": check_uri,
    "template_id": check_template_id,
}
