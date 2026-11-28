#!/usr/bin/env python3
"""Measure the real resource fees of each user operation and model TTL costs.

For every user lifecycle operation the script builds the same `stellar contract
invoke` command the smoke test uses, runs it with `--send=no` (simulate only, so
nothing is submitted and no state changes), and parses the simulation result:
the resource fee, the inclusion fee, the ledger footprint and the TTL
extensions the host would apply. TTL extension costs are modelled from
`src/constants.rs` (bump amount / lifetime threshold, in ledgers).

Usage:
  scripts/cost_model.py                 measure against deployments/testnet.json
  scripts/cost_model.py --dry-run       parse a recorded RPC fixture, no network
  scripts/cost_model.py --out docs/cost-model.md

Env:
  DEPLOYMENT   deployment file (default deployments/testnet.json)
  COST_MODEL_FIXTURE  fixture used by --dry-run
                      (default scripts/fixtures/cost-model-rpc.json)

The script is idempotent: it uses a fresh Friendbot-funded identity per run and
only ever simulates, so repeated runs produce the same table.
Standard library only.
"""
import json
import os
import pathlib
import re
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_DEPLOYMENT = ROOT / "deployments" / "testnet.json"
DEFAULT_FIXTURE = ROOT / "scripts" / "fixtures" / "cost-model-rpc.json"
DEFAULT_OUT = ROOT / "docs" / "cost-model.md"
CONSTANTS = ROOT / "src" / "constants.rs"

# Operations in user-lifecycle order. `write` operations change state (and so
# pay for instance TTL extension); `read` operations are simulated only.
OPERATIONS = [
    ("claim_folder", "write"),
    ("mint_portfolio", "write"),
    ("update_portfolio", "write"),
    ("set_user_template", "write"),
    ("verify_portfolio", "read"),
    ("get_portfolio", "read"),
    ("get_folder", "read"),
    ("get_identity", "read"),
    ("get_user_template", "read"),
    ("list_templates", "read"),
]


def read_constants(path: pathlib.Path = CONSTANTS) -> dict:
    """Parse `pub const NAME: u32 = <expr>;` from src/constants.rs."""
    text = path.read_text()
    raw = dict(re.findall(r"pub const (\w+): u32 = ([^;]+);", text))
    values: dict = {}

    def resolve(expr: str) -> int:
        expr = expr.strip()
        if expr.isdigit():
            return int(expr)
        if expr in values:
            return values[expr]
        total = 0
        for term in expr.split("+"):
            term = term.strip()
            if term.isdigit():
                total += int(term)
            elif term in values:
                total += values[term]
            else:
                raise ValueError(f"cannot resolve constant expression: {expr}")
        return total

    for name in raw:
        values[name] = resolve(raw[name])
    return values


def parse_simulation(payload: dict) -> dict:
    """Extract fees, footprint and TTL extensions from a simulateTransaction result."""
    result = payload.get("result", payload)
    if "error" in result:
        raise ValueError(f"simulation error: {result['error']}")
    cost = result.get("cost", {})
    footprint = result.get("transactionData", {}).get("resources", {})
    read_only = footprint.get("readOnly", [])
    read_write = footprint.get("readWrite", [])
    return {
        "resource_fee": int(cost.get("resourceFee", 0)),
        "inclusion_fee": int(cost.get("inclusionFee", 0)),
        "cpu_insns": int(cost.get("cpuInsns", 0)),
        "mem_bytes": int(cost.get("memBytes", 0)),
        "read_entries": len(read_only),
        "write_entries": len(read_write),
        "min_resource_fee": int(result.get("minResourceFee", 0)),
        "latest_ledger": int(result.get("latestLedger", 0)),
    }


def ttl_model(constants: dict) -> dict:
    """Model the per-entry TTL extension cost from src/constants.rs."""
    day = constants["DAY_IN_LEDGERS"]
    persistent_bump = constants["PERSISTENT_BUMP_AMOUNT"]
    persistent_threshold = constants["PERSISTENT_LIFETIME_THRESHOLD"]
    instance_bump = constants["INSTANCE_BUMP_AMOUNT"]
    instance_threshold = constants["INSTANCE_LIFETIME_THRESHOLD"]
    return {
        "day_in_ledgers": day,
        "persistent_bump_ledgers": persistent_bump,
        "persistent_threshold_ledgers": persistent_threshold,
        "instance_bump_ledgers": instance_bump,
        "instance_threshold_ledgers": instance_threshold,
        "persistent_bump_days": persistent_bump / day,
        "persistent_threshold_days": persistent_threshold / day,
        "instance_bump_days": instance_bump / day,
        "instance_threshold_days": instance_threshold / day,
        # Extensions happen at most once per (bump - threshold) ledgers.
        "persistent_extension_period_days": (persistent_bump - persistent_threshold) / day,
        "instance_extension_period_days": (instance_bump - instance_threshold) / day,
    }


def render_markdown(meta: dict, rows: list, ttl: dict) -> str:
    lines = [
        "# Cost Model",
        "",
        "Generated by [`scripts/cost_model.py`](../scripts/cost_model.py). Every row is a",
        "`simulateTransaction` (`--send=no`) of the operation, so no state is changed.",
        "",
        f"- Network: `{meta['network']}` (`{meta['rpc_url']}`)",
        f"- Contract: `{meta['contract_id']}`",
        f"- Latest ledger: `{meta['latest_ledger']}`",
        f"- Protocol version: `{meta['protocol_version']}`",
        f"- Stellar CLI: `{meta['cli_version']}`",
        f"- Source: `{meta['source']}`",
        "",
        "## Per-operation resource fees (stroops)",
        "",
        "| Operation | Kind | Resource fee | Inclusion fee | CPU insns | Mem bytes | Read entries | Write entries |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            "| `{name}` | {kind} | {resource_fee} | {inclusion_fee} | {cpu_insns} | "
            "{mem_bytes} | {read_entries} | {write_entries} |".format(**row)
        )
    lines += [
        "",
        "## TTL extension model (`src/constants.rs`)",
        "",
        "| Storage | Extended to | Re-extended below | Extension period |",
        "|---|---|---|---|",
        f"| Instance + code | {ttl['instance_bump_days']:.0f} days "
        f"({ttl['instance_bump_ledgers']} ledgers) | {ttl['instance_threshold_days']:.0f} days "
        f"({ttl['instance_threshold_ledgers']} ledgers) | "
        f"~{ttl['instance_extension_period_days']:.0f} days |",
        f"| Persistent | {ttl['persistent_bump_days']:.0f} days "
        f"({ttl['persistent_bump_ledgers']} ledgers) | {ttl['persistent_threshold_days']:.0f} days "
        f"({ttl['persistent_threshold_ledgers']} ledgers) | "
        f"~{ttl['persistent_extension_period_days']:.0f} days |",
        "",
        f"One day is {ttl['day_in_ledgers']} ledgers. A write operation pays for the instance",
        "extension at most once per extension period; a persistent entry pays for its own",
        "extension at most once per extension period, on the next read or write that finds it",
        "below the threshold.",
        "",
    ]
    return "\n".join(lines)


def run(cmd: list, check: bool = True) -> str:
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if check and proc.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} failed: {proc.stderr.strip()}")
    return proc.stdout


def measure_live(deployment: dict, identity: str) -> tuple:
    net_args = [
        "--network", deployment["network"],
        "--rpc-url", deployment["rpc_url"],
        "--network-passphrase", deployment["network_passphrase"],
    ]
    contract = deployment["contract_id"]
    template_id = str(deployment["templates"][0]["template_id"])
    address = run(["stellar", "keys", "address", identity]).strip()
    folder = f"cost-{int(time.time())}.folder"
    cid = "bafkreiejxg6d3gqvi3737mcwxkv4u7gismaraxgv5zfoqtfuc3nbdgve7i"
    args = {
        "claim_folder": ["--owner", address, "--folder_name", folder],
        "mint_portfolio": ["--owner", address, "--template_id", template_id, "--ipfs_cid", cid],
        "update_portfolio": ["--owner", address, "--ipfs_cid", cid],
        "set_user_template": ["--owner", address, "--template_id", template_id],
        "verify_portfolio": ["--owner", address],
        "get_portfolio": ["--owner", address],
        "get_folder": ["--folder_name", folder],
        "get_identity": ["--owner", address],
        "get_user_template": ["--owner", address],
        "list_templates": [],
    }
    rows = []
    for name, kind in OPERATIONS:
        out = run(
            ["stellar", "contract", "invoke", "--id", contract, "--source", identity,
             *net_args, "--send=no", "--", name, *args[name]]
        )
        row = {"name": name, "kind": kind}
        row.update(parse_simulation(json.loads(out)))
        rows.append(row)
    return rows, address


def main(argv: list) -> int:
    dry_run = "--dry-run" in argv
    out_path = DEFAULT_OUT
    if "--out" in argv:
        out_path = pathlib.Path(argv[argv.index("--out") + 1])
    deployment_path = pathlib.Path(os.environ.get("DEPLOYMENT", DEFAULT_DEPLOYMENT))
    deployment = json.loads(deployment_path.read_text())
    ttl = ttl_model(read_constants())

    if dry_run:
        fixture = json.loads(
            pathlib.Path(os.environ.get("COST_MODEL_FIXTURE", DEFAULT_FIXTURE)).read_text()
        )
        rows = []
        for name, kind in OPERATIONS:
            if name not in fixture["simulations"]:
                raise SystemExit(f"fixture is missing a simulation for {name}")
            row = {"name": name, "kind": kind}
            row.update(parse_simulation(fixture["simulations"][name]))
            rows.append(row)
        meta = {
            "network": fixture["network"],
            "rpc_url": fixture["rpc_url"],
            "contract_id": fixture["contract_id"],
            "latest_ledger": fixture["latest_ledger"],
            "protocol_version": fixture["protocol_version"],
            "cli_version": fixture["cli_version"],
            "source": "recorded RPC fixture (dry run)",
        }
    else:
        identity = f"cost-model-{int(time.time())}"
        run(["stellar", "keys", "generate", identity, "--network", deployment["network"],
             "--rpc-url", deployment["rpc_url"],
             "--network-passphrase", deployment["network_passphrase"], "--fund"])
        rows, address = measure_live(deployment, identity)
        info = json.loads(run(["stellar", "network", "settings", "--network", deployment["network"],
                               "--rpc-url", deployment["rpc_url"]], check=False) or "{}")
        meta = {
            "network": deployment["network"],
            "rpc_url": deployment["rpc_url"],
            "contract_id": deployment["contract_id"],
            "latest_ledger": max(r["latest_ledger"] for r in rows),
            "protocol_version": info.get("protocol_version", "unknown"),            "cli_version": run(["stellar", "--version"]).strip(),
            "source": f"fresh Friendbot identity `{address}`",
        }

    markdown = render_markdown(meta, rows, ttl)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(markdown)
    print(markdown)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
