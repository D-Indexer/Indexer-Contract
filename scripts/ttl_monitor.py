#!/usr/bin/env python3
"""Report and extend TTLs of the contract's critical ledger entries.

Reads `liveUntilLedgerSeq` for the contract instance, the wasm code entry,
`DataKey::TemplateList` and every `DataKey::Template(id)` via the RPC
`getLedgerEntries` method, prints the days remaining and runs
`stellar contract extend` for entries below `--threshold-days`.

Usage:
  scripts/ttl_monitor.py --threshold-days 30 --extend-days 90 \
      [--deployment deployments/testnet.json] [--source deployer] [--dry-run]
"""

import argparse
import base64
import json
import os
import struct
import subprocess
import sys
import urllib.request

LEDGER_SECONDS = 5
LEDGERS_PER_DAY = 86400 // LEDGER_SECONDS

# XDR discriminants (Stellar-ledger-entries.x / Stellar-contract.x)
LEDGER_KEY_CONTRACT_DATA = 6
LEDGER_KEY_CONTRACT_CODE = 7
SC_ADDRESS_TYPE_CONTRACT = 1
SCV_U32 = 3
SCV_SYMBOL = 15
SCV_VEC = 16
SCV_LEDGER_KEY_CONTRACT_INSTANCE = 20
DURABILITY_PERSISTENT = 1

_B32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"


def _u32(n):
    return struct.pack(">I", n)


def _opaque(b):
    return _u32(len(b)) + b + b"\x00" * (-len(b) % 4)


def decode_contract_id(strkey):
    """Return the 32-byte contract hash from a `C...` strkey."""
    bits = "".join(format(_B32.index(c), "05b") for c in strkey)
    raw = bytes(int(bits[i : i + 8], 2) for i in range(0, len(bits) - 7, 8))
    if len(raw) != 35 or raw[0] != 2 << 3:
        raise ValueError(f"not a contract strkey: {strkey}")
    return raw[1:33]


def scval_symbol(s):
    return _u32(SCV_SYMBOL) + _opaque(s.encode())


def scval_u32(n):
    return _u32(SCV_U32) + _u32(n)


def scval_vec(items):
    return _u32(SCV_VEC) + _u32(1) + _u32(len(items)) + b"".join(items)


def scval_instance():
    return _u32(SCV_LEDGER_KEY_CONTRACT_INSTANCE)


def datakey_template_list():
    return scval_vec([scval_symbol("TemplateList")])


def datakey_template(template_id):
    return scval_vec([scval_symbol("Template"), scval_u32(template_id)])


def ledger_key_contract_data(contract_id, key_scval):
    return (
        _u32(LEDGER_KEY_CONTRACT_DATA)
        + _u32(SC_ADDRESS_TYPE_CONTRACT)
        + decode_contract_id(contract_id)
        + key_scval
        + _u32(DURABILITY_PERSISTENT)
    )


def ledger_key_contract_code(wasm_hash_hex):
    return _u32(LEDGER_KEY_CONTRACT_CODE) + bytes.fromhex(wasm_hash_hex)


def b64(b):
    return base64.b64encode(b).decode()


def build_targets(deployment):
    """Return [(label, ledger_key_b64, extend_args)] for the critical entries."""
    cid = deployment["contract_id"]
    targets = [
        ("instance", ledger_key_contract_data(cid, scval_instance()), ["--id", cid]),
        ("code", ledger_key_contract_code(deployment["wasm_hash"]), ["--wasm-hash", deployment["wasm_hash"]]),
    ]
    keys = [("TemplateList", datakey_template_list())]
    keys += [(f"Template({t['template_id']})", datakey_template(t["template_id"])) for t in deployment.get("templates", [])]
    for label, scval in keys:
        targets.append(
            (label, ledger_key_contract_data(cid, scval),
             ["--id", cid, "--key-xdr", b64(scval), "--durability", "persistent"])
        )
    return [(label, b64(key), args) for label, key, args in targets]


def rpc_get_ledger_entries(rpc_url, keys):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "getLedgerEntries", "params": {"keys": keys}}).encode()
    req = urllib.request.Request(rpc_url, body, {"Content-Type": "application/json", "User-Agent": "indexer-contract-ttl-monitor"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.load(resp)
    if "error" in data:
        raise RuntimeError(f"RPC error: {data['error']}")
    return data["result"]


def evaluate(targets, result, threshold_days):
    """Return rows of (label, live_until, days_left, needs_extend). Missing entries have live_until None."""
    latest = result["latestLedger"]
    live = {e["key"]: e.get("liveUntilLedgerSeq") for e in result.get("entries") or []}
    rows = []
    for label, key, _ in targets:
        until = live.get(key)
        days = None if until is None else (until - latest) / LEDGERS_PER_DAY
        rows.append((label, until, days, days is not None and days < threshold_days))
    return rows


def render_table(rows, threshold_days):
    lines = [
        f"### Ledger entry TTLs (threshold {threshold_days} days)",
        "",
        "| entry | live_until_ledger | days remaining | action |",
        "|---|---|---|---|",
    ]
    for label, until, days, extend in rows:
        if until is None:
            lines.append(f"| {label} | missing | - | ⚠️ archived or absent (restore required) |")
        else:
            lines.append(f"| {label} | {until} | {days:.1f} | {'extend' if extend else 'ok'} |")
    return "\n".join(lines) + "\n"


def extend(args, deployment, extend_args, run=subprocess.run):
    cmd = [
        "stellar", "contract", "extend", *extend_args,
        "--ledgers-to-extend", str(int(args.extend_days * LEDGERS_PER_DAY)),
        "--source", args.source,
        "--rpc-url", deployment["rpc_url"],
        "--network-passphrase", deployment["network_passphrase"],
    ]
    print("+", " ".join(cmd), file=sys.stderr)
    if not args.dry_run:
        run(cmd, check=True)


def main(argv=None, rpc=rpc_get_ledger_entries, run=subprocess.run):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--deployment", default="deployments/testnet.json")
    p.add_argument("--threshold-days", type=float, required=True)
    p.add_argument("--extend-days", type=float, required=True)
    p.add_argument("--source", default=os.environ.get("STELLAR_ACCOUNT", "deployer"))
    p.add_argument("--dry-run", action="store_true", help="report only; print extend commands without running them")
    args = p.parse_args(argv)

    with open(args.deployment) as f:
        deployment = json.load(f)
    targets = build_targets(deployment)
    rows = evaluate(targets, rpc(deployment["rpc_url"], [t[1] for t in targets]), args.threshold_days)

    table = render_table(rows, args.threshold_days)
    print(table)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as f:
            f.write(table)

    for (label, _, extend_args), (_, _, _, needs) in zip(targets, rows):
        if needs:
            extend(args, deployment, extend_args, run)
    return 1 if any(until is None for _, until, _, _ in rows) else 0


if __name__ == "__main__":
    sys.exit(main())
