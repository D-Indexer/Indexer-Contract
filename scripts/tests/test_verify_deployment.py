import contextlib
import io
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import verify_deployment as verifier  # noqa: E402


RECORD = {
    "contract_id": "C" + "A" * 55,
    "source_commit": "a" * 40,
    "wasm_hash": "1" * 64,
    "toolchain": {"rustc": "rustc 1.99.1", "stellar_cli": "stellar 28.1.0"},
}


class VerifyDeployment(unittest.TestCase):
    def test_hash_comparison_accepts_matching_hash_and_rejects_changed_wasm(self):
        expected = "a" * 64
        self.assertTrue(verifier.hashes_match(expected, expected))
        self.assertFalse(verifier.hashes_match(expected, "b" * 64))

    def test_hash_comparison_rejects_invalid_hashes(self):
        with self.assertRaisesRegex(ValueError, "64 hexadecimal"):
            verifier.hashes_match("not-a-hash", "a" * 64)

    def test_toolchain_mismatch_fails(self):
        output = io.StringIO()
        with contextlib.redirect_stderr(output):
            self.assertFalse(
                verifier.check_toolchain(RECORD, "rustc 2.0.0", "stellar 28.1.0")
            )
        self.assertIn("rustc version mismatch", output.getvalue())

    def test_missing_deployment_fields_fail(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            path = pathlib.Path(temp_dir) / "deployment.json"
            path.write_text(json.dumps({"contract_id": RECORD["contract_id"]}))
            with self.assertRaisesRegex(ValueError, "source_commit"):
                verifier.load_deployment(path)


if __name__ == "__main__":
    unittest.main()
