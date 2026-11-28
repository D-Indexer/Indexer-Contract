#!/usr/bin/env bash
# Tests pin-templates.sh --verify-only against a mocked HTTP gateway:
# success, sha256 mismatch and timeout.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PORT="${MOCK_PORT:-18734}"
python3 - "$PORT" <<'PY' &
import http.server, json, sys, time
files = {m["cid"]: open(m["file"], "rb").read() for m in json.load(open("templates/manifest.json"))}
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        mode, _, cid = self.path.strip("/").split("/", 2)
        if mode == "slow":
            time.sleep(3)
        body = files.get(cid, b"")
        if mode == "bad":
            body = b"tampered" + body
        try:
            self.send_response(200); self.end_headers(); self.wfile.write(body)
        except BrokenPipeError:
            pass  # client timed out (expected in the timeout case)
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
PY
SERVER=$!
trap 'kill $SERVER 2>/dev/null' EXIT
for _ in $(seq 50); do curl -s "http://127.0.0.1:$PORT/ok/ipfs/x" >/dev/null && break; sleep 0.1; done

BASE="http://127.0.0.1:$PORT"
export RETRIES=2 FETCH_TIMEOUT=1
fail=0
expect() { # expect <exit code> <name> <gateways>
  local code=0
  GATEWAYS="$3" ./scripts/pin-templates.sh --verify-only >/dev/null 2>&1 || code=$?
  if [[ "$code" == "$1" ]]; then echo "ok   $2"; else echo "FAIL $2 (exit $code, want $1)"; fail=1; fi
}
expect 0 success  "$BASE/ok $BASE/ok"
expect 1 mismatch "$BASE/ok $BASE/bad"
expect 1 timeout  "$BASE/ok $BASE/slow"
exit "$fail"
