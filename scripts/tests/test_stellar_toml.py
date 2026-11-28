"""Run with: python3 -m unittest discover -s scripts/tests"""
import pathlib
import sys
import tomllib
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import stellar_toml  # noqa: E402

FIXTURES = pathlib.Path(__file__).resolve().parent / "fixtures" / "stellar_toml"


class StellarTomlTest(unittest.TestCase):
    def setUp(self):
        self.config = stellar_toml.load_config(FIXTURES / "config.json")
        self.deployments = stellar_toml.load_deployments(FIXTURES / "deployments")

    def test_generation_is_deterministic_and_sorted(self):
        first = stellar_toml.render(self.deployments, self.config)
        again = stellar_toml.render(list(reversed(self.deployments)), self.config)
        self.assertEqual(first, again)
        entries = tomllib.loads(first)["FOLDER_CONTRACTS"]
        self.assertEqual([e["network"] for e in entries], ["mainnet", "testnet"])
        self.assertEqual(entries[1]["contract_id"], "CTESTNET")
        self.assertEqual(entries[1]["documentation"], "https://example.com/docs")
        self.assertNotIn("deployed_at", entries[1])

    def test_generated_fragment_verifies_clean(self):
        text = stellar_toml.render(self.deployments, self.config)
        published = 'VERSION = "2.7.0"\n\n' + text
        self.assertEqual(stellar_toml.compare(published, self.deployments, self.config), [])

    def test_verify_reports_mismatches(self):
        published = (FIXTURES / "published_stale.toml").read_text()
        problems = stellar_toml.compare(published, self.deployments, self.config)
        self.assertIn(
            "mainnet: contract_id is 'COLDMAINNET', expected 'CMAINNET'", problems
        )
        self.assertIn("testnet: missing from [[FOLDER_CONTRACTS]]", problems)
        self.assertIn("futurenet: published but has no deployment record", problems)

    def test_verify_reports_unparseable_toml(self):
        problems = stellar_toml.compare("not = [toml", self.deployments, self.config)
        self.assertTrue(problems[0].startswith("stellar.toml does not parse"))

    def test_committed_fragment_is_current(self):
        self.assertEqual(stellar_toml.main(["generate", "--check"]), 0)


if __name__ == "__main__":
    unittest.main()
