#!/usr/bin/env python3
"""Validate deployment records against deployments/schema.json.

Usage: validate_deployments.py [FILE ...]   (default: every deployments/*.json)

Implements the JSON Schema 2020-12 keywords the schema uses ($ref to $defs,
type, enum, required, properties, items, pattern, minLength, minimum) with the
standard library only, so CI and scripts/deploy.sh need no extra packages.
Exits non-zero if any record is invalid.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_PATH = ROOT / "deployments" / "schema.json"

TYPES = {
    "object": lambda v: isinstance(v, dict),
    "array": lambda v: isinstance(v, list),
    "string": lambda v: isinstance(v, str),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "null": lambda v: v is None,
}


def validate(value, schema, root=None, path="$"):
    """Return a list of error strings for `value` against `schema`."""
    root = schema if root is None else root
    if "$ref" in schema:
        ref = schema["$ref"]
        if not ref.startswith("#/$defs/"):
            raise ValueError(f"unsupported $ref {ref}")
        return validate(value, root["$defs"][ref[len("#/$defs/"):]], root, path)

    if "type" in schema:
        types = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
        if not any(TYPES[t](value) for t in types):
            return [f"{path}: expected {' or '.join(types)}, got {json.dumps(value)}"]
    errors = []
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{path}: {json.dumps(value)} is not one of {schema['enum']}")
    if isinstance(value, str):
        if "minLength" in schema and len(value) < schema["minLength"]:
            errors.append(f"{path}: shorter than {schema['minLength']}")
        if "pattern" in schema and not re.search(schema["pattern"], value):
            errors.append(f"{path}: {value!r} does not match {schema['pattern']}")
    if TYPES["number"](value) and "minimum" in schema and value < schema["minimum"]:
        errors.append(f"{path}: {value} is less than {schema['minimum']}")
    if isinstance(value, dict):
        for key in schema.get("required", []):
            if key not in value:
                errors.append(f"{path}: missing required property {key!r}")
        for key, sub in schema.get("properties", {}).items():
            if key in value:
                errors += validate(value[key], sub, root, f"{path}.{key}")
    if isinstance(value, list) and "items" in schema:
        for i, item in enumerate(value):
            errors += validate(item, schema["items"], root, f"{path}[{i}]")
    return errors


def main(argv):
    schema = json.loads(SCHEMA_PATH.read_text())
    files = [Path(a) for a in argv] or sorted(
        p for p in (ROOT / "deployments").glob("*.json") if p.name != "schema.json"
    )
    failed = False
    for path in files:
        try:
            errors = validate(json.loads(path.read_text()), schema)
        except (OSError, json.JSONDecodeError) as exc:
            errors = [str(exc)]
        for err in errors:
            print(f"{path}: {err}", file=sys.stderr)
        failed |= bool(errors)
        if not errors:
            print(f"{path}: ok")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
