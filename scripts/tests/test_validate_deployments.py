"""Tests for scripts/validate_deployments.py and deployments/schema.json."""

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).resolve().parent / "fixtures" / "deployments"
sys.path.insert(0, str(ROOT / "scripts"))

import validate_deployments  # noqa: E402

SCHEMA = json.loads((ROOT / "deployments" / "schema.json").read_text())


def errors(path):
    return validate_deployments.validate(json.loads(path.read_text()), SCHEMA)


class SchemaTest(unittest.TestCase):
    def test_committed_records_validate(self):
        records = [p for p in (ROOT / "deployments").glob("*.json") if p.name != "schema.json"]
        self.assertTrue(records)
        for path in records:
            self.assertEqual(errors(path), [], path)

    def test_valid_fixture(self):
        self.assertEqual(errors(FIXTURES / "valid.json"), [])

    def test_invalid_fixtures(self):
        expected = {
            "invalid_contract_id.json": "$.contract_id",
            "invalid_missing_wasm_hash.json": "missing required property 'wasm_hash'",
            "invalid_tx_hash.json": "$.transactions.deploy",
            "invalid_admin.json": "$.admin",
            "invalid_deploy_ledger.json": "$.deploy_ledger",
        }
        for name, message in expected.items():
            with self.subTest(name=name):
                found = errors(FIXTURES / name)
                self.assertTrue(any(message in e for e in found), found)

    def test_cli_exit_code(self):
        self.assertEqual(validate_deployments.main([str(FIXTURES / "valid.json")]), 0)
        self.assertEqual(validate_deployments.main([str(FIXTURES / "invalid_admin.json")]), 1)


if __name__ == "__main__":
    unittest.main()
