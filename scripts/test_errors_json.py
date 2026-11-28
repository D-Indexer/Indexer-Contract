#!/usr/bin/env python3
"""Tests for scripts/errors_json.py. Run: python3 scripts/test_errors_json.py"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import errors_json  # noqa: E402

SAMPLE = """
pub enum ContractError {
    /// First thing.
    /// User message: Friendly first.
    First = 1,
    /// Second thing.
    /// User message: Friendly second.
    Second = 2,
}
"""


class ErrorsJsonTest(unittest.TestCase):
    def test_parses_variants(self):
        self.assertEqual(
            errors_json.parse(SAMPLE),
            [
                {"code": 1, "name": "First", "description": "First thing.", "user_message": "Friendly first."},
                {"code": 2, "name": "Second", "description": "Second thing.", "user_message": "Friendly second."},
            ],
        )

    def test_missing_doc_comment_fails(self):
        with self.assertRaises(ValueError):
            errors_json.parse(SAMPLE.replace("    /// Second thing.\n    /// User message: Friendly second.\n", ""))

    def test_missing_user_message_fails(self):
        with self.assertRaises(ValueError):
            errors_json.parse(SAMPLE.replace("    /// User message: Friendly first.\n", ""))

    def test_matches_real_enum(self):
        errors = errors_json.parse(errors_json.SRC.read_text())
        codes = [e["code"] for e in errors]
        self.assertEqual(codes, list(range(1, len(codes) + 1)))


if __name__ == "__main__":
    unittest.main()
