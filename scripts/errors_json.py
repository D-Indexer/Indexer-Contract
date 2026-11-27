#!/usr/bin/env python3
"""Generate errors.json from the ContractError enum in src/errors.rs.

Each variant must carry `///` doc comments: a description, then a line
starting with `User message:`.

  python3 scripts/errors_json.py          # write errors.json
  python3 scripts/errors_json.py --check  # fail if errors.json is stale
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src" / "errors.rs"
OUT = ROOT / "errors.json"
USER_PREFIX = "User message:"

VARIANT = re.compile(r"^\s*([A-Z][A-Za-z0-9]*)\s*=\s*(\d+)\s*,?\s*$")


def parse(text):
    body = text.split("pub enum ContractError", 1)[1]
    body = body[body.index("{") + 1 : body.index("}")]
    errors, docs = [], []
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.startswith("///"):
            docs.append(stripped[3:].strip())
            continue
        match = VARIANT.match(line)
        if not match:
            continue
        name, code = match.group(1), int(match.group(2))
        description = " ".join(d for d in docs if d and not d.startswith(USER_PREFIX))
        user = [d[len(USER_PREFIX) :].strip() for d in docs if d.startswith(USER_PREFIX)]
        if not description or not user or not user[0]:
            raise ValueError(
                f"{name}: needs a doc comment description and a '{USER_PREFIX}' line"
            )
        errors.append(
            {"code": code, "name": name, "description": description, "user_message": user[0]}
        )
        docs = []
    if not errors:
        raise ValueError("no ContractError variants found")
    return errors


def render(errors):
    return json.dumps(errors, indent=2) + "\n"


def main(argv):
    try:
        expected = render(parse(SRC.read_text()))
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    if "--check" in argv:
        if not OUT.exists() or OUT.read_text() != expected:
            print("errors.json is stale; run: python3 scripts/errors_json.py", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(expected)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
