#!/usr/bin/env python3
"""Look up the ledgers of deployment transactions.

Usage:
  backfill_ledgers.py ledger HASH --rpc-url URL [--network NET]
      Print the ledger HASH was included in (used by scripts/deploy.sh).
  backfill_ledgers.py backfill FILE [--rpc-url URL] [--network NET]
      Add `deploy_ledger`, `ledgers` and templates[].ledger to a deployment
      record in place, from the tx hashes it already contains.

The ledger comes from RPC `getTransaction`. RPC only keeps recent history, so a
NOT_FOUND answer falls back to Horizon, which keeps every transaction.
"""

import argparse
import json
import sys
import time
import urllib.error
import urllib.request

HORIZON = {
    "testnet": "https://horizon-testnet.stellar.org",
    "mainnet": "https://horizon.stellar.org",
}
TX_STEPS = ("upload_wasm", "deploy", "initialize")


def _request(url, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json",
                                                       "User-Agent": "indexer-contract-deploy"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)


def rpc_ledger(tx_hash, rpc_url, request=_request):
    """Ledger from RPC getTransaction, or None if RPC does not have the tx."""
    resp = request(rpc_url, {"jsonrpc": "2.0", "id": 1, "method": "getTransaction",
                             "params": {"hash": tx_hash}})
    if "error" in resp:
        raise LookupError(f"getTransaction {tx_hash}: {resp['error']}")
    result = resp["result"]
    if result["status"] == "NOT_FOUND":
        return None
    if result["status"] != "SUCCESS":
        raise LookupError(f"transaction {tx_hash} has status {result['status']}")
    return int(result["ledger"])


def horizon_ledger(tx_hash, horizon_url, request=_request):
    try:
        resp = request(f"{horizon_url}/transactions/{tx_hash}")
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return None
        raise
    if not resp.get("successful", True):
        raise LookupError(f"transaction {tx_hash} failed")
    return int(resp["ledger"])


def lookup_ledger(tx_hash, rpc_url, network=None, request=_request, retries=5, delay=2.0):
    for attempt in range(retries):
        ledger = rpc_ledger(tx_hash, rpc_url, request)
        if ledger is None and network in HORIZON:
            ledger = horizon_ledger(tx_hash, HORIZON[network], request)
        if ledger is not None:
            return ledger
        if attempt + 1 < retries:
            time.sleep(delay)
    raise LookupError(f"transaction {tx_hash} not found")


def _is_hash(value):
    return isinstance(value, str) and len(value) == 64


def backfill(record, lookup):
    """Return `record` with ledgers filled in by `lookup(hash) -> int`."""
    record = dict(record)
    txs = record["transactions"]
    record["ledgers"] = {
        step: lookup(txs[step]) if _is_hash(txs.get(step)) else None for step in TX_STEPS
    }
    record["deploy_ledger"] = record["ledgers"]["deploy"]
    record["templates"] = [
        {**t, "ledger": lookup(t["tx_hash"]) if _is_hash(t.get("tx_hash")) else None}
        for t in record.get("templates", [])
    ]
    return record


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=["ledger", "backfill"])
    parser.add_argument("target", help="tx hash (ledger) or deployment file (backfill)")
    parser.add_argument("--rpc-url")
    parser.add_argument("--network")
    args = parser.parse_args(argv)

    if args.command == "ledger":
        if not args.rpc_url:
            parser.error("--rpc-url is required for 'ledger'")
        print(lookup_ledger(args.target, args.rpc_url, args.network))
        return 0

    with open(args.target) as f:
        record = json.load(f)
    rpc_url = args.rpc_url or record["rpc_url"]
    network = args.network or record["network"]
    record = backfill(record, lambda h: lookup_ledger(h, rpc_url, network, retries=1))
    with open(args.target, "w") as f:
        json.dump(record, f, indent=2)
        f.write("\n")
    print(f"{args.target}: deploy_ledger {record['deploy_ledger']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
