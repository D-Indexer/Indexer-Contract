#!/usr/bin/env python3
"""Generate and diff a normalised snapshot of the contract interface.

The snapshot is produced from `stellar contract info interface --output json`
run against the optimized wasm, then normalised (sorted, stable key order) so
that diffs only reflect real interface changes. Standard library only.

Usage:
  scripts/interface_snapshot.py generate --wasm <path> --out interface.snapshot.json
  scripts/interface_snapshot.py diff --base <old.json> --head <new.json> [--breaking-ok]

`diff` classifies every change as additive or breaking and exits non-zero when a
breaking change is found and `--breaking-ok` is not passed.

Breaking changes:
  - removed function, struct, enum, error enum, event, or type alias
  - removed or reordered function parameter, changed parameter name/type
  - changed return type, added a required parameter
  - removed struct field, changed field type, reordered fields
  - removed enum variant, changed variant value
  - removed error variant, changed error code
  - removed event field, changed event field type, reordered event fields
Additive changes:
  - new function, struct, enum, error enum, event, type alias
  - new struct field appended at the end
  - new enum variant appended at the end
  - new error variant appended at the end
  - new event field appended at the end
"""
import argparse
import json
import pathlib
import subprocess
import sys

SNAPSHOT_VERSION = 1


def run_interface(wasm: pathlib.Path) -> dict:
    proc = subprocess.run(
        ["stellar", "contract", "info", "interface", "--wasm", str(wasm), "--output", "json"],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(proc.stdout)


def _sorted_by_name(items):
    return sorted(items or [], key=lambda item: item.get("name", ""))


def _normalise_function(fn: dict) -> dict:
    return {
        "name": fn.get("name"),
        "inputs": [
            {"name": arg.get("name"), "type": arg.get("type")}
            for arg in fn.get("inputs") or fn.get("args") or []
        ],
        "outputs": [
            {"type": out.get("type")} for out in fn.get("outputs") or []
        ],
        "doc": fn.get("doc") or "",
    }


def _normalise_struct(item: dict) -> dict:
    return {
        "name": item.get("name"),
        "fields": [
            {"name": f.get("name"), "type": f.get("type")} for f in item.get("fields") or []
        ],
        "doc": item.get("doc") or "",
    }


def _normalise_enum(item: dict) -> dict:
    return {
        "name": item.get("name"),
        "cases": [
            {"name": c.get("name"), "value": c.get("value")} for c in item.get("cases") or []
        ],
        "doc": item.get("doc") or "",
    }


def _normalise_error(item: dict) -> dict:
    return {
        "name": item.get("name"),
        "cases": [
            {"name": c.get("name"), "value": c.get("value")} for c in item.get("cases") or []
        ],
        "doc": item.get("doc") or "",
    }


def _normalise_event(item: dict) -> dict:
    return {
        "name": item.get("name"),
        "fields": [
            {"name": f.get("name"), "type": f.get("type")} for f in item.get("fields") or []
        ],
        "doc": item.get("doc") or "",
    }


def _normalise_alias(item: dict) -> dict:
    return {"name": item.get("name"), "type": item.get("type"), "doc": item.get("doc") or ""}


def normalise(raw: dict) -> dict:
    """Return a deterministic, sorted snapshot of the interface."""
    return {
        "snapshot_version": SNAPSHOT_VERSION,
        "functions": [
            _normalise_function(fn) for fn in _sorted_by_name(raw.get("functions"))
        ],
        "structs": [_normalise_struct(s) for s in _sorted_by_name(raw.get("structs"))],
        "enums": [_normalise_enum(e) for e in _sorted_by_name(raw.get("enums"))],
        "error_enums": [
            _normalise_error(e) for e in _sorted_by_name(raw.get("error_enums"))
        ],
        "events": [_normalise_event(e) for e in _sorted_by_name(raw.get("events"))],
        "type_aliases": [
            _normalise_alias(a) for a in _sorted_by_name(raw.get("type_aliases"))
        ],
    }


def _index(items):
    return {item["name"]: item for item in items or []}


def _diff_ordered_fields(kind: str, name: str, base: list, head: list, changes: list) -> None:
    """Compare ordered field/case lists; appends are additive, the rest breaking."""
    base_names = [f["name"] for f in base]
    head_names = [f["name"] for f in head]
    head_by_name = _index(head)
    base_by_name = _index(base)

    for field in base:
        if field["name"] not in head_by_name:
            changes.append(("breaking", f"{kind} `{name}`: removed field `{field['name']}`"))
        elif head_by_name[field["name"]] != field:
            changes.append(
                ("breaking", f"{kind} `{name}`: changed field `{field['name']}`"
                 f" ({field} -> {head_by_name[field['name']]})")
            )
        elif field["name"] not in head_names:
            changes.append(("breaking", f"{kind} `{name}`: removed field `{field['name']}`"))

    common_base = [n for n in base_names if n in head_by_name]
    common_head = [n for n in head_names if n in base_by_name]
    if common_base != common_head:
        changes.append(
            ("breaking", f"{kind} `{name}`: reordered fields ({common_base} -> {common_head})")
        )

    for field in head:
        if field["name"] not in base_by_name:
            if base_names and head_names.index(field["name"]) < len(base_names):
                changes.append(
                    ("breaking", f"{kind} `{name}`: inserted field `{field['name']}` before the end")
                )
            else:
                changes.append(("additive", f"{kind} `{name}`: added field `{field['name']}`"))


def _diff_named(kind: str, base: list, head: list, changes: list, compare) -> None:
    base_by_name = _index(base)
    head_by_name = _index(head)
    for name, item in base_by_name.items():
        if name not in head_by_name:
            changes.append(("breaking", f"{kind} `{name}`: removed"))
        else:
            compare(kind, name, item, head_by_name[name], changes)
    for name in head_by_name:
        if name not in base_by_name:
            changes.append(("additive", f"{kind} `{name}`: added"))


def _compare_function(kind, name, base, head, changes):
    base_inputs = base["inputs"]
    head_inputs = head["inputs"]
    if len(head_inputs) < len(base_inputs):
        changes.append(("breaking", f"function `{name}`: removed parameters"))
    elif len(head_inputs) > len(base_inputs):
        changes.append(("breaking", f"function `{name}`: added required parameters"))
    for old, new in zip(base_inputs, head_inputs):
        if old != new:
            changes.append(
                ("breaking", f"function `{name}`: parameter changed ({old} -> {new})")
            )
    if base["outputs"] != head["outputs"]:
        changes.append(
            ("breaking", f"function `{name}`: return type changed"
             f" ({base['outputs']} -> {head['outputs']})")
        )


def _compare_struct(kind, name, base, head, changes):
    _diff_ordered_fields("struct", name, base["fields"], head["fields"], changes)


def _compare_enum(kind, name, base, head, changes):
    _diff_ordered_fields("enum", name, base["cases"], head["cases"], changes)


def _compare_error(kind, name, base, head, changes):
    _diff_ordered_fields("error enum", name, base["cases"], head["cases"], changes)


def _compare_event(kind, name, base, head, changes):
    _diff_ordered_fields("event", name, base["fields"], head["fields"], changes)


def _compare_alias(kind, name, base, head, changes):
    if base["type"] != head["type"]:
        changes.append(
            ("breaking", f"type alias `{name}`: changed ({base['type']} -> {head['type']})")
        )


def diff(base: dict, head: dict) -> list:
    changes: list = []
    _diff_named("function", base.get("functions"), head.get("functions"), changes, _compare_function)
    _diff_named("struct", base.get("structs"), head.get("structs"), changes, _compare_struct)
    _diff_named("enum", base.get("enums"), head.get("enums"), changes, _compare_enum)
    _diff_named("error enum", base.get("error_enums"), head.get("error_enums"), changes, _compare_error)
    _diff_named("event", base.get("events"), head.get("events"), changes, _compare_event)
    _diff_named("type alias", base.get("type_aliases"), head.get("type_aliases"), changes, _compare_alias)
    return changes


def cmd_generate(args) -> int:
    snapshot = normalise(run_interface(pathlib.Path(args.wasm)))
    out = pathlib.Path(args.out)
    out.write_text(json.dumps(snapshot, indent=2, sort_keys=True) + "\n")
    print(f"wrote {out}")
    return 0


def cmd_diff(args) -> int:
    base = json.loads(pathlib.Path(args.base).read_text())
    head = json.loads(pathlib.Path(args.head).read_text())
    changes = diff(base, head)

    breaking = [msg for kind, msg in changes if kind == "breaking"]
    additive = [msg for kind, msg in changes if kind == "additive"]

    for msg in additive:
        print(f"additive: {msg}")
    for msg in breaking:
        print(f"BREAKING: {msg}")

    if not changes:
        print("interface unchanged")
        return 0
    if breaking and not args.breaking_ok:
        print(
            f"\n{len(breaking)} breaking change(s) detected. "
            "Add the `breaking-change` label to the PR to allow this.",
            file=sys.stderr,
        )
        return 1
    if breaking:
        print(f"\n{len(breaking)} breaking change(s) allowed by the `breaking-change` label.")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    gen = sub.add_parser("generate", help="generate a normalised interface snapshot")
    gen.add_argument("--wasm", required=True)
    gen.add_argument("--out", default="interface.snapshot.json")
    gen.set_defaults(func=cmd_generate)

    dif = sub.add_parser("diff", help="classify the diff between two snapshots")
    dif.add_argument("--base", required=True)
    dif.add_argument("--head", required=True)
    dif.add_argument("--breaking-ok", action="store_true")
    dif.set_defaults(func=cmd_diff)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
