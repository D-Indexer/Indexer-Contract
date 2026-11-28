"""Tests for scripts/backfill_ledgers.py against recorded RPC/Horizon responses."""

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RPC = Path(__file__).resolve().parent / "fixtures" / "rpc"
sys.path.insert(0, str(ROOT / "scripts"))

import backfill_ledgers  # noqa: E402


def fixture(name):
    return json.loads((RPC / name).read_text())


def requester(rpc_fixture, horizon_fixture="horizon_transaction.json"):
    def request(url, body=None):
        return fixture(rpc_fixture if body is not None else horizon_fixture)
    return request


class BackfillTest(unittest.TestCase):
    def test_rpc_success(self):
        ledger = backfill_ledgers.lookup_ledger("ab" * 32, "http://rpc", "testnet",
                                                requester("get_transaction_success.json"))
        self.assertEqual(ledger, 5647752)

    def test_not_found_falls_back_to_horizon(self):
        ledger = backfill_ledgers.lookup_ledger("ab" * 32, "http://rpc", "testnet",
                                                requester("get_transaction_not_found.json"))
        self.assertEqual(ledger, 5647760)

    def test_not_found_without_horizon_raises(self):
        with self.assertRaises(LookupError):
            backfill_ledgers.lookup_ledger("ab" * 32, "http://rpc", None,
                                           requester("get_transaction_not_found.json"), retries=1)

    def test_failed_transaction_raises(self):
        with self.assertRaises(LookupError):
            backfill_ledgers.rpc_ledger("ab" * 32, "http://rpc", requester("get_transaction_failed.json"))

    def test_backfill_record(self):
        record = json.loads((ROOT / "deployments" / "testnet.json").read_text())
        for key in ("deploy_ledger", "ledgers"):
            record.pop(key)
        ledgers = {h: i for i, h in enumerate(
            list(record["transactions"].values()) + [t["tx_hash"] for t in record["templates"]], 100)}
        out = backfill_ledgers.backfill(record, ledgers.__getitem__)
        self.assertEqual(out["ledgers"], {"upload_wasm": 100, "deploy": 101, "initialize": 102})
        self.assertEqual(out["deploy_ledger"], 101)
        self.assertEqual([t["ledger"] for t in out["templates"]], [103, 104, 105])

    def test_already_uploaded_has_no_ledger(self):
        record = {"transactions": {"upload_wasm": "already-uploaded", "deploy": "ab" * 32,
                                   "initialize": "cd" * 32}, "templates": []}
        out = backfill_ledgers.backfill(record, lambda h: 7)
        self.assertIsNone(out["ledgers"]["upload_wasm"])


if __name__ == "__main__":
    unittest.main()
