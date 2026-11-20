"""Run with: python3 -m unittest discover -s scripts/tests

get_events.json holds events emitted by the real contract in a testutils Env
(topics/values are the contract's own XDR), wrapped in a getEvents response.
"""
import base64
import json
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import admin_monitor as m  # noqa: E402

FIXTURE = pathlib.Path(__file__).resolve().parent / "fixtures" / "admin_monitor" / "get_events.json"
ADMIN = "GCFADTAQUXYFXUW7G65YXIQTTDORMT72FFTUYFEEUXJZDNAYTD2RXMBZ"
ROGUE = "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"
THIRD = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7"


def load():
    return json.loads(FIXTURE.read_text())["result"]


class DecodeTest(unittest.TestCase):
    def setUp(self):
        self.decoded = [m.decode_event(e) for e in load()["events"]]

    def test_privileged_events_decode_with_named_fields(self):
        names = [d and d["event"] for d in self.decoded]
        self.assertEqual(
            names,
            ["init", "adm_add", "adm_add", "adm_rm", "tmpl_reg", "treas_set", "tmpl_price", None],
        )
        self.assertEqual(self.decoded[1]["details"], {"admin": ADMIN, "new_admin": ROGUE})
        self.assertEqual(self.decoded[3]["details"]["target"], ROGUE)
        reg = self.decoded[4]
        self.assertEqual(reg["actor"], ADMIN)
        self.assertEqual(reg["details"]["template_id"], 1)
        self.assertTrue(reg["details"]["metadata_uri"].startswith("ipfs://bafkrei"))
        price = self.decoded[6]["details"]["price"]
        self.assertEqual((price["amount"], price["designer_bps"]), (3, 5000))
        self.assertTrue(price["token"].startswith("C"))

    def test_contract_strkey_matches_deployment(self):
        # Contract hash taken from the tool's LedgerKey for the testnet instance.
        key = base64.b64decode("AAAABgAAAAHHBX+SYfkQ8PhZ2eDjL5gYNkUSPbsNf4oA65qUffq5twAAABQAAAAB")
        self.assertEqual(
            m.strkey(2 << 3, key[8:40]),
            "CDDQK74SMH4RB4HYLHM6BYZPTAMDMRISHW5Q274KADVZVFD57K43O6KV",
        )

    def test_future_privileged_event_uses_first_address(self):
        event = {"topic": [base64.b64encode(b"\0\0\0\x0f\0\0\0\x07upgrade\0").decode()],
                 "value": load()["events"][0]["value"]}
        decoded = m.decode_event(event)
        self.assertEqual((decoded["event"], decoded["actor"]), ("upgrade", ADMIN))

    def test_garbage_is_ignored(self):
        self.assertIsNone(m.decode_event({"topic": ["AAAA"], "value": ""}))


class RulesTest(unittest.TestCase):
    def setUp(self):
        self.alerts = [m.evaluate(d, {ADMIN}) for d in map(m.decode_event, load()["events"]) if d]

    def test_expected_admin_actions_are_normal(self):
        for i in (0, 4, 5, 6):
            self.assertEqual(self.alerts[i]["priority"], "normal", self.alerts[i]["event"])

    def test_granting_unknown_admin_is_high(self):
        self.assertEqual(self.alerts[1]["priority"], "high")
        self.assertIn(f"unexpected address {ROGUE} was granted admin", self.alerts[1]["reasons"])

    def test_unknown_actor_is_high(self):
        alert = self.alerts[2]
        self.assertEqual(alert["priority"], "high")
        self.assertIn(f"actor {ROGUE} is not an expected admin", alert["reasons"])
        self.assertIn(f"unexpected address {THIRD} was granted admin", alert["reasons"])

    def test_allowlisted_new_admin_is_normal(self):
        alert = m.evaluate(m.decode_event(load()["events"][1]), {ADMIN, ROGUE})
        self.assertEqual(alert["priority"], "normal")

    def test_render_includes_explorer_links(self):
        title, body = m.render_alert(self.alerts[2], "testnet", "CXYZ")
        self.assertTrue(title.startswith("[HIGH] privileged `adm_add`"))
        self.assertIn("https://stellar.expert/explorer/testnet/tx/" + "02" * 32, body)
        self.assertIn("https://stellar.expert/explorer/testnet/contract/CXYZ", body)
        self.assertIn(f"https://stellar.expert/explorer/testnet/account/{ROGUE}", body)


class CursorTest(unittest.TestCase):
    def run_fetch(self, state, oldest=5707641, latest=5828610):
        calls = []

        def fake_rpc(url, method, params=None):
            calls.append((method, params))
            if method == "getHealth":
                return {"oldestLedger": oldest, "latestLedger": latest}
            return load()

        original, m.rpc = m.rpc, fake_rpc
        try:
            result = m.fetch_events("rpc", "CID", state)
        finally:
            m.rpc = original
        return result, calls[1][1]

    def test_resumes_from_stored_cursor(self):
        (events, state, gap), params = self.run_fetch({"cursor": "abc", "ledger": 5800000})
        self.assertEqual(params["pagination"]["cursor"], "abc")
        self.assertNotIn("startLedger", params)
        self.assertIsNone(gap)
        self.assertEqual(len(events), 8)
        self.assertEqual(state, {"cursor": load()["cursor"], "ledger": 5828610})

    def test_first_run_looks_back(self):
        (_, _, gap), params = self.run_fetch({})
        self.assertEqual(params["startLedger"], 5828610 - m.FIRST_RUN_LOOKBACK)
        self.assertIsNone(gap)

    def test_cursor_outside_retention_reports_gap(self):
        (_, _, gap), params = self.run_fetch({"cursor": "old", "ledger": 5000000})
        self.assertEqual(params["startLedger"], 5707641)
        self.assertEqual(gap, (5000001, 5707641))
        self.assertEqual(m.gap_alert(*gap)["priority"], "high")


if __name__ == "__main__":
    unittest.main()
