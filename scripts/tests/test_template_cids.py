import contextlib
import hashlib
import io
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import template_cids as cids  # noqa: E402


class TemplateHashes(unittest.TestCase):
    def test_hash_manifest_check_rejects_mismatched_sha256(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            templates = root / "templates"
            templates.mkdir()
            data = b'{"name":"test"}\n'
            (templates / "test.json").write_bytes(data)
            entry = {
                "template_id": 1,
                "file": "templates/test.json",
                "cid": cids.cid_v1_raw(data),
                "metadata_uri": f"ipfs://{cids.cid_v1_raw(data)}",
                "content_sha256": hashlib.sha256(data).hexdigest(),
            }
            manifest = templates / "manifest.json"
            manifest.write_text(json.dumps([entry]))
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(cids.check(manifest), 0)

            entry["content_sha256"] = "0" * 64
            manifest.write_text(json.dumps([entry]))
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(cids.check(manifest), 1)


if __name__ == "__main__":
    unittest.main()
