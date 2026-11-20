import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import ttl_monitor as m  # noqa: E402

CID = "CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV"
WASM = "83fa464a26a7827a4b2a2b8d23f5a28d8c62a0536c0f373f42c76631ade2396b"

# Known-good fixtures generated with the official stellar-sdk (py) XDR encoder.
FIXTURES = {
    "instance": "AAAABgAAAAHHBX+SYfkQ8PhZ2eDjL5gYNkUSPbsNf4oA65qUffq5twAAABQAAAAB",
    "code": "AAAAB4P6Rkomp4J6SyorjSP1oo2MYqBTbA83P0LHZjGt4jlr",
    "TemplateList": "AAAABgAAAAHHBX+SYfkQ8PhZ2eDjL5gYNkUSPbsNf4oA65qUffq5twAAABAAAAABAAAAAQAAAA8AAAAMVGVtcGxhdGVMaXN0AAAAAQ==",
    "Template(3)": "AAAABgAAAAHHBX+SYfkQ8PhZ2eDjL5gYNkUSPbsNf4oA65qUffq5twAAABAAAAABAAAAAgAAAA8AAAAIVGVtcGxhdGUAAAADAAAAAwAAAAE=",
}
DEPLOYMENT = {
    "contract_id": CID,
    "wasm_hash": WASM,
    "rpc_url": "http://rpc.invalid",
    "network_passphrase": "Test SDF Network ; September 2015",
    "templates": [{"template_id": 3}],
}


class KeyEncoding(unittest.TestCase):
    def test_contract_id(self):
        self.assertEqual(m.decode_contract_id(CID).hex(), "c7057f9261f910f0f859d9e0e32f98183645123dbb0d7f8a00eb9a947dfab9b7")

    def test_ledger_keys_match_fixtures(self):
        keys = {label: key for label, key, _ in m.build_targets(DEPLOYMENT)}
        self.assertEqual(keys, FIXTURES)

    def test_template_list_scval(self):
        self.assertEqual(m.b64(m.datakey_template_list()), "AAAAEAAAAAEAAAABAAAADwAAAAxUZW1wbGF0ZUxpc3Q=")


class ThresholdLogic(unittest.TestCase):
    def run_main(self, threshold, live_days, missing=()):
        latest = 1000
        rpc = lambda url, keys: {  # noqa: E731
            "latestLedger": latest,
            "entries": [
                {"key": k, "liveUntilLedgerSeq": latest + int(live_days * m.LEDGERS_PER_DAY)}
                for label, k in zip(FIXTURES, keys)
                if label not in missing
            ],
        }
        calls = []
        os.environ.pop("GITHUB_STEP_SUMMARY", None)
        rc = m.main(
            ["--deployment", self.path, "--threshold-days", str(threshold), "--extend-days", "90", "--source", "ops"],
            rpc=rpc,
            run=lambda cmd, check: calls.append(cmd),
        )
        return rc, calls

    def setUp(self):
        import json
        import tempfile

        fd, self.path = tempfile.mkstemp(suffix=".json")
        with os.fdopen(fd, "w") as f:
            json.dump(DEPLOYMENT, f)

    def tearDown(self):
        os.unlink(self.path)

    def test_no_extension_above_threshold(self):
        rc, calls = self.run_main(threshold=10, live_days=20)
        self.assertEqual((rc, calls), (0, []))

    def test_extends_all_when_threshold_high(self):
        rc, calls = self.run_main(threshold=10000, live_days=20)
        self.assertEqual(rc, 0)
        self.assertEqual(len(calls), 4)
        self.assertIn("--wasm-hash", calls[1])
        self.assertIn(str(90 * m.LEDGERS_PER_DAY), calls[0])
        self.assertEqual(calls[2][calls[2].index("--key-xdr") + 1], "AAAAEAAAAAEAAAABAAAADwAAAAxUZW1wbGF0ZUxpc3Q=")

    def test_missing_entry_fails_without_extending_it(self):
        rc, calls = self.run_main(threshold=10, live_days=20, missing=("TemplateList",))
        self.assertEqual((rc, calls), (1, []))


if __name__ == "__main__":
    unittest.main()
