"""Scenario tests for scripts/deploy.sh against a mocked `stellar` CLI.

Each test runs the script in a throwaway copy of the repository with
scripts/tests/mock_bin first on PATH and a local stub RPC for ledger lookups.
"""

import json
import os
import shutil
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MOCK_BIN = Path(__file__).resolve().parent / "mock_bin"
LEDGER = 4242
FIRST_CONTRACT = "C" + "A" * 55
ADMIN = "GBMOCKADMINAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"


class _Rpc(BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        body = json.dumps({"jsonrpc": "2.0", "id": 1,
                           "result": {"status": "SUCCESS", "ledger": LEDGER}}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


class DeployScriptTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rpc = ThreadingHTTPServer(("127.0.0.1", 0), _Rpc)
        threading.Thread(target=cls.rpc.serve_forever, daemon=True).start()

    @classmethod
    def tearDownClass(cls):
        cls.rpc.shutdown()

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.root = self.tmp / "repo"
        self.chain = self.tmp / "chain"
        self.chain.mkdir()
        (self.root / "scripts").mkdir(parents=True)
        (self.root / "deployments").mkdir()
        for name in ("deploy.sh", "validate_deployments.py", "backfill_ledgers.py", "template_cids.py"):
            shutil.copy(REPO / "scripts" / name, self.root / "scripts" / name)
        shutil.copytree(REPO / "templates", self.root / "templates")
        shutil.copy(REPO / "deployments" / "schema.json", self.root / "deployments")
        git = ["git", "-c", "user.name=t", "-c", "user.email=t@example.com"]
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        subprocess.run(git + ["commit", "-q", "--allow-empty", "-m", "init"], cwd=self.root, check=True)
        self.manifest = json.loads((REPO / "templates" / "manifest.json").read_text())

    def deploy(self, *args, ok=True, **env):
        full_env = {**os.environ, "PATH": f"{MOCK_BIN}:{os.environ['PATH']}",
                    "MOCK_DIR": str(self.chain),
                    "STELLAR_RPC_URL": f"http://127.0.0.1:{self.rpc.server_address[1]}", **env}
        proc = subprocess.run(["bash", "scripts/deploy.sh", "testnet", *args], cwd=self.root,
                              env=full_env, capture_output=True, text=True, timeout=120)
        if ok != (proc.returncode == 0):
            self.fail(f"exit {proc.returncode}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}")
        return proc

    def record(self):
        return json.loads((self.root / "deployments" / "testnet.json").read_text())

    def state(self):
        return json.loads((self.root / "deploy-out" / "testnet" / "state.json").read_text())

    def deploys(self):
        return int((self.chain / "count.deploy").read_text())

    def assert_complete(self):
        record = self.record()
        self.assertEqual(record["contract_id"], FIRST_CONTRACT)
        self.assertEqual(record["deploy_ledger"], LEDGER)
        self.assertEqual([t["template_id"] for t in record["templates"]],
                         [t["template_id"] for t in self.manifest])
        self.assertEqual(self.deploys(), 1)
        out = subprocess.run(["python3", "scripts/validate_deployments.py", "deployments/testnet.json"],
                             cwd=self.root, capture_output=True, text=True)
        self.assertEqual(out.returncode, 0, out.stderr)
        return record

    def test_full_deploy_writes_valid_record(self):
        proc = self.deploy()
        record = self.assert_complete()
        self.assertIn(f"DEPLOY_LEDGER={LEDGER}", proc.stdout)
        self.assertEqual(record["ledgers"], {"upload_wasm": LEDGER, "deploy": LEDGER, "initialize": LEDGER})

    def test_resume_completes_on_same_contract_after_each_interruption(self):
        for point in ("upload", "deploy", "initialize", "register_template", "register_template:2"):
            with self.subTest(point=point):
                self.setUp()
                self.deploy(ok=False, MOCK_FAIL_ON=point)
                self.assertIn("pass --resume", self.deploy(ok=False).stderr)
                self.deploy("--resume")
                self.assert_complete()

    def test_resume_after_initialize_skips_completed_steps(self):
        self.deploy(ok=False, MOCK_FAIL_ON="register_template")
        self.assertIn("initialize", self.state()["steps"])
        self.deploy("--resume")
        record = self.assert_complete()
        self.assertEqual(record["contract_id"], self.state()["contract_id"])

    def test_template_uri_mismatch_fails_loudly(self):
        self.deploy(ok=False, MOCK_FAIL_ON="register_template")
        first = self.manifest[0]["template_id"]
        (self.chain / "templates").write_text(f"{first}\tipfs://wrong\n")
        proc = self.deploy("--resume", ok=False)
        self.assertIn(f"template {first} is registered with ipfs://wrong", proc.stderr)

    def test_fresh_deploys_new_contract(self):
        self.deploy()
        (self.chain / "admins").unlink()
        (self.chain / "templates").write_text("")
        self.deploy("--fresh")
        self.assertEqual(self.record()["contract_id"], "C" + "B" * 55)

    def test_build_only_and_submit_one_step_at_a_time(self):
        env = {"ADMIN_ADDRESS": ADMIN}
        self.assertIn("ADMIN_ADDRESS", self.deploy("--build-only", ok=False).stderr)
        self.deploy("--build-only", **env)
        out = self.root / "deploy-out" / "testnet"
        self.assertEqual((out / "upload_wasm.xdr").read_text().strip(), "XDR-upload")
        summary = (out / "upload_wasm.txt").read_text()
        self.assertIn("operation: upload wasm", summary)
        self.assertIn("fee:       12345 stroops", summary)

        signed = self.tmp / "signed.xdr"
        signed.write_text("SIGNED-upload\n")
        self.assertIn("Next: deploy", self.deploy("--submit", str(signed), **env).stdout)
        self.assertEqual(self.state()["steps"]["upload_wasm"]["ledger"], LEDGER)
        self.assertNotIn("pending", self.state())

        self.deploy("--build-only", **env)
        self.assertEqual(self.state()["contract_id"], "C" + "Z" * 55)
        self.assertEqual(self.state()["pending"], "deploy")
        self.assertTrue((out / "deploy.xdr").exists())


if __name__ == "__main__":
    unittest.main()
